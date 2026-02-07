# zDirComp.exe — Specification

Pure Rust CLI สำหรับ Windows 10/11 ใช้ร่วมกับ uTorrent/BitTorrent เพื่อ:
1. **ลบไฟล์เกิน** ที่ไม่อยู่ใน torrent ออกจากโฟลเดอร์ดาวน์โหลด
2. **ปลดล็อกไฟล์** โดย terminate process ที่ล็อกไฟล์ (ยกเว้น uTorrent/BitTorrent)

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
| `<directory>` | Path ถึงโฟลเดอร์ดาวน์โหลด | `E:\Online\MyFiles` |

### ลำดับการทำงาน

```
1. ซ่อนหน้าต่าง console
2. ตรวจสอบ Safety Guard — path ต้องลึกอย่างน้อย 3 ระดับ
3. Walk ทุกไฟล์ในโฟลเดอร์ (recursive)
4. เรียก Win32 Restart Manager API:
   a. RmStartSession()
   b. RmRegisterResources() — ลงทะเบียนไฟล์ทั้งหมด
   c. RmGetList() — ดึงรายชื่อ process ที่ล็อกไฟล์
   d. กรอง process — ข้าม uTorrent.exe และ BitTorrent.exe
   e. terminate เฉพาะ process ที่ไม่ใช่ torrent client (ใช้ TerminateProcess)
   f. RmEndSession()
5. เขียน log สรุปผล
```

### Trigger

ผ่าน uTorrent → **"Run this program when a torrent changes state:"**

เมื่อ torrent เข้าสถานะ **Finding Peers (23)** → แสดงว่ากำลังจะเริ่มดาวน์โหลด → ปลดล็อกไฟล์ก่อน

> **สำคัญ:** uTorrent เรียก command ทุกครั้งที่ state เปลี่ยน ไม่ว่า state จะเป็นอะไร ดังนั้นต้องกรอง `%S=="23"` ในตัว command เอง (ดูหัวข้อ [การตั้งค่า uTorrent](#การตั้งค่า-utorrent))

### Process Exclusion

เนื่องจากเราไม่สามารถใช้ `RmShutdown` ได้โดยตรง (จะ terminate uTorrent ด้วย) จึงต้อง:
1. ใช้ `RmGetList` ดึงรายชื่อ process ที่ล็อกไฟล์
2. วนลูปตรวจชื่อทีละ process
3. ข้าม uTorrent.exe / BitTorrent.exe
4. terminate ด้วย `TerminateProcess` เฉพาะ process ที่ไม่อยู่ใน exclusion list

| Process | จัดการ |
|---|---|
| `uTorrent.exe` | ❌ **ข้าม** — ห้าม terminate เด็ดขาด |
| `BitTorrent.exe` | ❌ **ข้าม** — ห้าม terminate เด็ดขาด |
| `explorer.exe` | ✅ terminate ได้ (Windows จะ restart ให้อัตโนมัติ) |
| process อื่น ๆ | ✅ terminate ได้ |

### ความปลอดภัย

| Guard | รายละเอียด |
|---|---|
| **Process exclusion** | ไม่ kill torrent client |
| **Depth check** | ป้องกัน path ตื้นเกินไป |
| **RAII session guard** | `RmEndSession()` ถูกเรียกเสมอ แม้เกิด error |
| **Error tolerance** | ถ้า terminate ไม่ได้ ก็ข้ามไป — ไม่ crash |
| **Hidden window** | ไม่แสดง console popup |

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
[2026-02-07 21:30:05] UNLOCK "E:\Online\MyTorrent" — killed 1 process (explorer.exe), skipped BitTorrent.exe
[2026-02-07 21:31:00] SYNC "E:\Mobile\B" — path too shallow, aborted
[2026-02-07 21:32:00] SYNC "E:\Online\Stuff" — torrent file not found, aborted
```

### กรณีที่ log

| เหตุการณ์ | ข้อความตัวอย่าง |
|---|---|
| Sync สำเร็จ | `SYNC "dir" — deleted N files, M empty dirs` |
| Sync ไม่มีอะไรลบ | `SYNC "dir" — clean, nothing to remove` |
| Unlock สำเร็จ | `UNLOCK "dir" — killed N processes (names), skipped M` |
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
| Window subsystem | `#![windows_subsystem = "windows"]` (ซ่อน console) |
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
| `RmGetList` | unlock | ดึงรายชื่อ process ที่ล็อกไฟล์ |
| `RmEndSession` | unlock | จบ session |
| `OpenProcess` | unlock | เปิด process handle |
| `TerminateProcess` | unlock | terminate process ที่ล็อก (selective) |
| `QueryFullProcessImageNameW` | unlock | อ่านชื่อ executable ของ process |
| `CloseHandle` | unlock | ปิด handle |

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
| Restart Manager error | เขียน log + ข้ามไป ทำต่อ |
| process terminate ไม่ได้ | เขียน log + ข้ามไป ทำต่อ |
