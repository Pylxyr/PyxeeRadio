## Installation

### Windows

Download **PyxeeRadio Setup** below and run it. It installs for your user only and keeps your settings in `%APPDATA%\TwitchRadio`.

**Windows SmartScreen:** the installer may show "Windows protected your PC" (see the signing note at the end). Choose **More info**, then **Run anyway**. To check the file first, compare its SHA-256 with `SHA256SUMS.txt` (`Get-FileHash <file>` in PowerShell). GitHub renames spaces in file names to dots, so the downloaded file is `PyxeeRadio.Setup.x.y.z.exe`.

### Linux (x86-64; built for Arch and CachyOS, should run on other recent distributions)

Download `PyxeeRadio-x.y.z-linux-x64.tar.gz`, then:

```
tar xzf PyxeeRadio-*-linux-x64.tar.gz
./PyxeeRadio-*-linux-x64/PyxeeRadio
```

No installer is needed, but the system must provide WebKitGTK 4.1 (`sudo pacman -S webkit2gtk-4.1`) and, for the tray icon, `libayatana-appindicator`. ffmpeg is bundled. Settings live in `~/.config/TwitchRadio`. Check downloads with `sha256sum -c --ignore-missing SHA256SUMS.txt`.
