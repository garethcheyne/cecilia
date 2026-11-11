# Cecilia on a Raspberry Pi

A Pi 4 or Pi 5 (2 GB or more) with 64-bit Raspberry Pi OS makes a good
Cecilia box. It lives at the organ, and anyone with a phone or tablet on the
church Wi-Fi can open Cecilia in a browser.

## What you need

- A Raspberry Pi 4 or 5, its power supply, and a 16 GB+ microSD card.
- A **USB MIDI interface** with one input and one output. Good choices are the
  Roland UM-ONE mk2 and ESI M4U eX. Cheap unbranded cables often drop or garble
  messages, so avoid them.
- Optionally, a small speaker or the organ's auxiliary input, if you want to
  hear the simulator from the Pi.

## Connecting a Makin

Use the sockets as labelled on the organ:

| Organ socket | MIDI interface |
| --- | --- |
| **MIDI SEQ** (out) | **IN** — this is what Cecilia records |
| **MIDI IN** | **OUT** — this is what Cecilia plays into |

Before recording, press the organ's **General Cancel** so Cecilia and the
organ agree that every stop is in.

## Installing

1. Install Raspberry Pi OS (64-bit) with Raspberry Pi Imager. In its settings,
   set a host name such as `cecilia`, turn on SSH, and enter the church Wi-Fi.
2. Copy the latest `cecilia_*_arm64.deb` from the project's Releases page to
   the Pi, then:

   ```sh
   sudo apt install ./cecilia_*_arm64.deb
   ```

   This installs and starts the `cecilia` service. It also starts again by
   itself after a power cut.
3. On a phone or laptop on the same network, open `http://cecilia.local:7070`.
4. Open **Settings**, set a **password**, then use **MIDI Setup** to choose
   the USB MIDI interface as Organ in and Organ out.
5. On **Organs**, copy the **Makin** template, then **Learn every stop**.

The Pi listens on the network because `/etc/default/cecilia` sets
`CECILIA_ADDR=0.0.0.0:7070`. Set the password **before** putting the Pi on a
network other people use.

## Looking after it

- **Backups:** use **Settings → Download a backup** now and then, and keep the
  file somewhere other than the Pi.
- **Updates:** install the newer `.deb` the same way. Your music, playlists and
  organs in `/var/lib/cecilia` are kept.
- **Logs:** `journalctl -u cecilia -f`. They're kept in memory, so they
  don't wear the SD card.
- **Restart:** `sudo systemctl restart cecilia`.

## A touchscreen at the console (optional)

With the official 7" display, Chromium can open Cecilia full screen at
start-up. Add this to `~/.config/labwc/autostart` (Raspberry Pi OS Bookworm):

```sh
chromium-browser --kiosk --noerrdialogs --disable-infobars http://localhost:7070/service/
```

Settings → Text size → **Larger** helps at arm's length.

## Building the package yourself

On a PC with Docker:

```sh
cd web && npm ci && npm run build && cd ../server
cargo install cross cargo-deb
cross build --release --target aarch64-unknown-linux-gnu -p organ-server
cargo deb -p organ-server --no-build --target aarch64-unknown-linux-gnu
```

Use `armv7-unknown-linux-gnueabihf` for 32-bit Raspberry Pi OS.
