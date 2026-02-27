# Claude Notify

Ứng dụng desktop giúp bạn nhận thông báo âm thanh và push notification khi Claude Code hoàn thành task hoặc đặt câu hỏi. App chạy trong system tray, tự động cấu hình hooks vào `~/.claude/settings.json`.

## Cài đặt

1. Tải file **`Claude Notify_2.0.0_x64-setup.exe`**
2. Chạy file setup — app sẽ cài vào thư mục user, không cần quyền Admin
3. Sau khi cài xong, app sẽ tự khởi chạy

## Sử dụng

### Mở app

- App chạy ẩn trong **system tray** (góc phải dưới thanh taskbar)
- **Double-click** icon tray để mở cửa sổ cài đặt
- **Chuột phải** icon tray → chọn "Open Settings" hoặc "Quit"

### Cấu hình

| Tùy chọn | Mô tả |
|---|---|
| **Enable Notifications** | Bật/tắt toàn bộ hook thông báo |
| **Start with Windows** | Tự khởi động cùng Windows |
| **Stop Sound** | Âm thanh phát khi Claude Code hoàn thành task (file `.wav`) |
| **Ask Sound** | Âm thanh phát khi Claude Code đặt câu hỏi cho bạn (file `.wav`) |
| **ntfy Topic** | Topic để nhận push notification trên điện thoại (tùy chọn) |

### Các bước cấu hình

1. Mở cửa sổ cài đặt (double-click icon tray)
2. Bật **Enable Notifications**
3. Chọn file âm thanh `.wav` cho Stop Sound và Ask Sound bằng nút **Browse**
   - Mặc định dùng âm thanh Windows tại `C:\Windows\Media\`
4. (Tùy chọn) Nhập **ntfy topic** để nhận thông báo trên điện thoại
5. Bấm **Save Settings**
6. **Khởi động lại Claude Code** để hooks có hiệu lực

### Nút Test

- Bấm **Test** cạnh Stop Sound / Ask Sound để nghe thử âm thanh
- Bấm **Test** cạnh ntfy topic để gửi thông báo thử lên điện thoại

### Push notification qua ntfy (tùy chọn)

1. Cài app **ntfy** trên điện thoại ([Android](https://play.google.com/store/apps/details?id=io.heckel.ntfy) / [iOS](https://apps.apple.com/app/ntfy/id1625396347))
2. Mở app ntfy → Subscribe → nhập topic name (ví dụ: `claude-myname`)
3. Trong Claude Notify, nhập cùng topic name đó vào ô **ntfy Topic**
4. Bấm **Test** để kiểm tra → điện thoại sẽ nhận được notification
5. Bấm **Save Settings**

## Cách hoạt động

App tự động ghi hooks vào file `~/.claude/settings.json`:

- **Stop hook**: Phát âm thanh + gửi ntfy khi Claude Code hoàn thành task
- **PreToolUse hook** (AskUserQuestion): Phát âm thanh + gửi ntfy khi Claude Code hỏi bạn
- **Notification hook**: Gửi ntfy cho các thông báo khác

Khi tắt notifications, hooks được backup vào `_hooksBackup` và khôi phục khi bật lại.

## Gỡ cài đặt

- Vào **Settings > Apps > Claude Notify > Uninstall**
- Hoặc chạy lại file setup và chọn Uninstall

## Yêu cầu hệ thống

- Windows 10/11 (64-bit)
- Claude Code CLI đã được cài đặt
