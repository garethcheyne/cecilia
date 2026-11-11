# Cecilia SD card image

`.github/workflows/release.yml` builds a ready-to-flash Raspberry Pi image on
every release: Raspberry Pi OS Lite (64-bit) plus Cecilia and nothing else.

- `stage-cecilia/` is a pi-gen stage run after the standard stages 0 to 2. It
  installs the arm64 `.deb` from the same release and enables the service.
- The machine is called `cecilia`, so it's at `http://cecilia.local:7070`.
- Set the username, password and Wi-Fi in Raspberry Pi Imager's
  OS customisation when writing the card. The defaults (`organist` /
  `cecilia`) must be changed; Cecilia's own password is set in its Settings.
