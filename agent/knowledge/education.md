# Education

## Profile and when to use it
Use `profile.apply` with `education` for schools, classrooms, universities, training centers and student laptops. It sets up GNOME with a calm focus layout, LibreOffice, Firefox, Thonny and VSCodium for coding, GCompris, Stellarium, GeoGebra and Anki, Jitsi and Zoom for video lessons, printing and local discovery, and the readable Lexend typeface on a warm light theme.

## Shared labs versus personal devices
Shared computer-lab machines usually need a clean state: `session.configure` with `ephemeral_home: true` creates a shared `guest` account whose files are erased on every boot. Personal student or teacher laptops must keep their files, so never enable it without asking. This is the main question worth asking for education requests.

## Exam lock-down
Exam mode (`session.configure` with `mode: exam` and an https `kiosk_url`) boots straight into a single full-screen browser pointed at the exam platform, and should be combined with `security.configure` `usb_storage: block`. Ask for the exam URL if the request does not include one.

## Accessibility in classrooms
Lexend and Atkinson Hyperlegible Next improve reading fluency; `accessibility.set` with `large_text` and `text_scale` helps younger readers, and `reduce_motion` helps students sensitive to animation.
