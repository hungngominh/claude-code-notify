# Claude Notify

Ứng dụng desktop giúp bạn nhận thông báo âm thanh, Windows toast và Google Chat khi Claude Code hoàn thành task hoặc đặt câu hỏi. App chạy trong system tray, tự động cấu hình hooks vào `~/.claude/settings.json`.

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
| **Windows Toast** | Bật/tắt popup thông báo Windows |
| **Stop Sound** | Âm thanh phát khi Claude Code hoàn thành task (file `.wav`) |
| **Ask Sound** | Âm thanh phát khi Claude Code đặt câu hỏi cho bạn (file `.wav`) |
| **Google Chat Webhook** | URL webhook để gửi thông báo vào Google Chat (tùy chọn) |

### Các bước cấu hình

1. Mở cửa sổ cài đặt (double-click icon tray)
2. Bật **Enable Notifications**
3. Chọn file âm thanh `.wav` cho Stop Sound và Ask Sound bằng nút **Browse**
   - Mặc định dùng âm thanh Windows tại `C:\Windows\Media\`
4. (Tùy chọn) Nhập **Google Chat webhook URL** để nhận thông báo qua Google Chat
5. (Tùy chọn) Bật **Windows Toast** để nhận popup thông báo trên desktop
5. Bấm **Save Settings**
6. **Khởi động lại Claude Code** để hooks có hiệu lực

### Nút Test

- Bấm **Test** cạnh Stop Sound / Ask Sound để nghe thử âm thanh
- Bấm **Test** cạnh Google Chat webhook để gửi thông báo thử vào Google Chat

### Google Chat webhook (tùy chọn)

1. Mở Google Chat → tạo Space hoặc chọn Space có sẵn
2. Vào **Settings > Integrations > Webhooks > Add webhook**
3. Đặt tên webhook, bấm Save → copy URL webhook
4. Trong Claude Notify, dán URL webhook vào ô **Google Chat Webhook**
5. Bấm **Test** để kiểm tra → Google Chat sẽ nhận được card thông báo
6. Bấm **Save Settings**

## Cách hoạt động

App tự động ghi hooks vào file `~/.claude/settings.json`:

- **Stop hook**: Phát âm thanh + gửi Google Chat + toast khi Claude Code hoàn thành task
- **PreToolUse hook** (AskUserQuestion): Phát âm thanh + gửi Google Chat + toast khi Claude Code hỏi bạn
- **Notification hook**: Phát âm thanh + gửi Google Chat + toast cho các thông báo khác

Khi tắt notifications, hooks được backup vào `_hooksBackup` và khôi phục khi bật lại.

## Gỡ cài đặt

- Vào **Settings > Apps > Claude Notify > Uninstall**
- Hoặc chạy lại file setup và chọn Uninstall

## Yêu cầu hệ thống

- Windows 10/11 (64-bit)
- Claude Code CLI đã được cài đặt
