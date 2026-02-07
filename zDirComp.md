# zDirComp.exe — Specification

Pure Rust CLI สำหรับ Windows 10/11 เพื่อ:
1. **ลบไฟล์เกิน** ที่ไม่อยู่ใน torrent ออกจากโฟลเดอร์ดาวน์โหลด (ใช้ร่วมกับ uTorrent/BitTorrent)
2. **ปลดล็อกไฟล์** โดย terminate ทุก process ที่ล็อกไฟล์ ผ่าน `RmShutdown(RmForceShutdown)` (ใช้งานผ่าน cmd ได้โดยตรง)

ทดแทน `zDirComp.jar` (Java) ได้ทันที — ไม่ต้องติดตั้ง JRE อีกต่อไป

---

## สารบัญ

- [วางไฟล์](#วางไฟล์)
- [Mode 1: Sync — ลบไฟล์เกิน](#mode-1-sync--ลบไฟล์เกิน)
- [Mode 2: Unlock — ปลดล็อกไฟล์](#mode-2-unlock--ปลดล็อกไฟล์)
- [Safety Guard: ตรวจสอบความลึกของ Path](#safety-guard-ตรวจสอบความลึกของ-path)
- [การตั้งค่า uTorrent](#การตั้งค่า-utorrent)
- [Logging](#logging)
- [สรุปเทคนิค](#สรุปเทคนิค)

---

## วางไฟล์

```
%localappdata%\AutoSync\BitTorrent\
├── zDirComp.exe         ← ตัว executable ใหม่ (Rust)
├── zDirComp.log         ← log file (สร้างอัตโนมัติ)
└── *.torrent            ← uTorrent's torrent file storage
```

> uTorrent ต้องตั้งค่า **Preferences → Directories → Store .torrents in:** เป็น `%localappdata%\AutoSync\BitTorrent\`

---

## Mode 1: Sync — ลบไฟล์เกิน

### CLI

```
zDirComp.exe sync <torrent_file> <directory>
```

| Argument | Description | Example |
|---|---|---|
| `<torrent_file>` | Path ถึงไฟล์ `.torrent` | `%localappdata%\AutoSync\BitTorrent\MyFiles.torrent` |
| `<directory>` | Path ถึงโฟลเดอร์ดาวน์โหลด | `E:\Online\MyFiles` |

### ลำดับการทำงาน

```
1. ซ่อนหน้าต่าง console (ไม่แสดงอะไรบนหน้าจอ)
2. หน่วงเวลา 3 วินาที (รอ file lock จาก uTorrent หลุด)
3. ตรวจสอบ Safety Guard — path ต้องลึกอย่างน้อย 3 ระดับ
4. อ่านไฟล์ .torrent → parse Bencode → ดึงรายชื่อไฟล์ทั้งหมด
5. สร้าง HashSet ของ relative path ที่ควรมี
6. Walk directory (depth-first, children before parents)
   - ไฟล์ที่ไม่อยู่ใน HashSet → ลบ
   - โฟลเดอร์เปล่า → ลบ (non-recursive remove_dir)
7. เขียน log สรุปผล
```

### Trigger

ผ่าน uTorrent → **"Run this program when a torrent finishes:"**

uTorrent จะเรียก command นี้อัตโนมัติเมื่อ torrent ดาวน์โหลดเสร็จ — ไม่จำเป็นต้องตรวจสอบ state เพิ่มเติมเพราะ hook นี้ทำงานเฉพาะตอนเสร็จอยู่แล้ว

### ความปลอดภัย

| Guard | รายละเอียด |
|---|---|
| **3s delay** | รอ uTorrent ปล่อย file handle |
| **Depth check** | ป้องกัน path ตื้นเกินไป (ดูหัวข้อ [Safety Guard](#safety-guard-ตรวจสอบความลึกของ-path)) |
| **non-recursive `remove_dir`** | ลบเฉพาะโฟลเดอร์เปล่าเท่านั้น |
| **Hidden window** | ไม่แสดง console popup รบกวนผู้ใช้ |

---

## Mode 2: Unlock — ปลดล็อกไฟล์

### CLI

```
zDirComp.exe unlock <directory>
```

| Argument | Description | Example |
|---|---|---|
| `<directory>` | Path ถึงโฟลเดอร์ที่ต้องการปลดล็อก | `E:\Online\MyFiles` |

### ลำดับการทำงาน

```
1. ตรวจสอบ Safety Guard — path ต้องลึกอย่างน้อย 3 ระดับ
2. Walk ทุกไฟล์ในโฟลเดอร์ (recursive)
3. เรียก Win32 Restart Manager API:
   a. RmStartSession()
   b. RmRegisterResources() — ลงทะเบียนไฟล์ทั้งหมด
   c. RmGetList() — ดึงจำนวน process ที่ล็อกไฟล์
   d. RmShutdown(RmForceShutdown) — terminate ทุก process ที่ล็อก
   e. RmEndSession() (เรียกอัตโนมัติผ่าน RAII Drop)
4. เขียน log สรุปผล
```

### การใช้งาน

ใช้งานผ่าน **cmd/terminal ได้โดยตรง** — ไม่จำเป็นต้องผ่าน uTorrent

```cmd
zDirComp.exe unlock "E:\Online\MyFiles"
```

> สามารถตั้งค่าผ่าน uTorrent ได้เช่นกัน (ดูหัวข้อ [การตั้งค่า uTorrent](#การตั้งค่า-utorrent))

### วิธีทำงาน: RmShutdown(RmForceShutdown)

ใช้ **Restart Manager** ตัวเดียวกับที่ rqbit ใช้ — ให้ Windows จัดการ terminate เอง:

1. RM ส่ง `WM_CLOSE` ให้ app ปิดตัวอย่าง graceful ก่อน
2. ถ้าไม่ตอบสนอง → force terminate
3. RM มี authority สูงกว่า `TerminateProcess` → จัดการ elevated process ได้ดีกว่า
4. **ไม่มี process exclusion** — terminate ทุก process ที่ล็อก (รวม torrent client ถ้ามี)

### ความปลอดภัย

| Guard | รายละเอียด |
|---|---|
| **Depth check** | ป้องกัน path ตื้นเกินไป |
| **RAII session guard** | `RmEndSession()` ถูกเรียกเสมอ แม้เกิด error |
| **RmForceShutdown** | graceful ก่อน → force เฉพาะที่ไม่ตอบสนอง |
| **Error tolerance** | ถ้า shutdown ไม่ได้ → เขียน log, ไม่ crash |

---

## Safety Guard: ตรวจสอบความลึกของ Path

ป้องกันไม่ให้ลบไฟล์ในโฟลเดอร์ชั้นบนโดยไม่ตั้งใจ

### กฎ

**Directory path ต้องลึกอย่างน้อย 3 ระดับ** (drive + 2 directories):

| Path | Component Count | ผ่าน? |
|---|---|---|
| `E:\` | 1 | ❌ root drive |
| `E:\Online` | 2 | ❌ root dir — อาจมี torrent อื่นอยู่ด้วย |
| `E:\Online\MyTorrent` | 3 | ✅ sub dir — ปลอดภัย |
| `E:\Online\Category\MyTorrent` | 4 | ✅ ลึกกว่า — ปลอดภัย |

### ตัวอย่าง

```
✅ ทำงาน:  E:\Online\A\*
✅ ทำงาน:  E:\Mobile\B\*  
✅ ทำงาน:  D:\Downloads\Torrents\MyStuff\*

❌ ไม่ทำงาน:  E:\Online\*     (ตื้นเกินไป — อาจลบไฟล์ torrent อื่น)
❌ ไม่ทำงาน:  E:\Mobile\*     (ตื้นเกินไป)
❌ ไม่ทำงาน:  E:\*            (root drive — อันตราย!)
```

ถ้า path ไม่ผ่าน → โปรแกรมจะจบทันทีและเขียน log (exit code 1)

---

## การตั้งค่า uTorrent

### Preferences → Advanced → Run Program

#### ช่อง 1: "Run this program when a torrent finishes:"

```bat
cmd /c start /b "" "%localappdata%\AutoSync\BitTorrent\zDirComp.exe" sync "%localappdata%\AutoSync\BitTorrent\%N.torrent" "%D"
```

| ส่วน | หน้าที่ |
|---|---|
| `cmd /c start /b ""` | รันแบบ background ไม่แสดงหน้าต่าง |
| `zDirComp.exe sync` | เรียกโหมด Sync |
| `%N.torrent` | ชื่อ torrent file (uTorrent variable) |
| `"%D"` | โฟลเดอร์ที่ดาวน์โหลดไว้ (uTorrent variable) |

#### ช่อง 2: "Run this program when a torrent changes state:"

```bat
cmd /c if "%S"=="23" start /b "" "%localappdata%\AutoSync\BitTorrent\zDirComp.exe" unlock "%D"
```

| ส่วน | หน้าที่ |
|---|---|
| `if "%S"=="23"` | กรองเฉพาะ state 23 (Finding Peers) — ก่อนจะเริ่มโหลด |
| `start /b ""` | รันแบบ background |
| `zDirComp.exe unlock` | เรียกโหมด Unlock |
| `"%D"` | โฟลเดอร์ดาวน์โหลด |

---

## Logging

Log file อยู่ที่ `zDirComp.log` ข้าง ๆ `.exe` — append ต่อท้ายเสมอ ไม่ลบ log เก่า

### รูปแบบ

```
[2026-02-07 21:30:00] SYNC "E:\Online\MyTorrent" — deleted 3 files, 1 empty dir
[2026-02-07 21:30:05] UNLOCK "E:\Online\MyTorrent" — terminated 2 locking process(es)
[2026-02-07 21:31:00] SYNC "E:\Mobile\B" — path too shallow, aborted
[2026-02-07 21:32:00] SYNC "E:\Online\Stuff" — torrent file not found, aborted
```

### กรณีที่ log

| เหตุการณ์ | ข้อความตัวอย่าง |
|---|---|
| Sync สำเร็จ | `SYNC "dir" — deleted N files, M empty dirs` |
| Sync ไม่มีอะไรลบ | `SYNC "dir" — clean, nothing to remove` |
| Unlock สำเร็จ | `UNLOCK "dir" — terminated N locking process(es)` |
| Unlock ไม่มี lock | `UNLOCK "dir" — no locking processes found` |
| Path ตื้นเกินไป | `MODE "dir" — path too shallow, aborted` |
| .torrent ไม่เจอ | `SYNC "file" — torrent file not found, aborted` |
| Bencode error | `SYNC "file" — invalid torrent format, aborted` |

---

## สรุปเทคนิค

### Build & Compile

| หัวข้อ | ค่า |
|---|---|
| ภาษา | Pure Rust |
| Target | `x86_64-pc-windows-msvc` |
| Icon | ใช้ icon default ของ Windows (ไม่ embed icon) |
| Console | แสดง output ได้ปกติ — ใช้ `start /b` ใน uTorrent command เพื่อซ่อน |
| Dependencies | **ไม่มี** external library — ใช้ `std` + Win32 FFI โดยตรง |
| Output | `zDirComp.exe` (single static binary) |

### Source Layout

```
Torrent-Directory-Comparison/
├── rust/
│   ├── src/
│   │   ├── main.rs        ← จุดเข้า + CLI parsing
│   │   ├── bencode.rs     ← Bencode parser (port จาก Java)
│   │   ├── sync.rs        ← Mode 1: Sync Extra Files
│   │   ├── unlock.rs      ← Mode 2: Kill Locking Processes (Win32 FFI)
│   │   ├── safety.rs      ← Path depth validation
│   │   └── logger.rs      ← Log file writer
│   └── Cargo.toml         ← Project manifest (no dependencies)
├── icon.ico
└── ...
```

> Source อยู่ในโฟลเดอร์ `rust/` เพื่อแยกออกจาก Java source เดิม

### Win32 API ที่ใช้ (FFI โดยตรง ไม่ใช้ crate)

| API | ใช้ใน | หน้าที่ |
|---|---|---|
| `RmStartSession` | unlock | เริ่ม Restart Manager session |
| `RmRegisterResources` | unlock | ลงทะเบียนไฟล์ที่ต้องการตรวจ |
| `RmGetList` | unlock | ดึงจำนวน process ที่ล็อกไฟล์ |
| `RmShutdown` | unlock | terminate ทุก process ที่ล็อก (RmForceShutdown) |
| `RmEndSession` | unlock | จบ session |

### Bencode Parser

Port จาก `BencodeSerializer.java` → Rust:
- รองรับ 4 types: Integer, ByteString, List, Dictionary
- ใช้ recursive descent parsing จาก `&[u8]` slice
- ดึง `info → files → path` สร้างรายชื่อไฟล์

### Error Handling

| สถานการณ์ | พฤติกรรม |
|---|---|
| ไฟล์ .torrent ไม่เจอ | เขียน log + exit code 1 |
| Bencode format ผิด | เขียน log + exit code 1 |
| Path ตื้นเกินไป | เขียน log + exit code 1 |
| ลบไฟล์ไม่ได้ (permission) | เขียน log + ข้ามไป ทำต่อ |
| RmShutdown ล้มเหลว | เขียน log + รายงาน error code |
| process terminate ไม่ได้ | เขียน log + ข้ามไป ทำต่อ |
