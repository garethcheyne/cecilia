# Reading printed music (OMR)

Registra can turn a scan or photo of printed music into a track. It uses
[Audiveris](https://github.com/Audiveris/audiveris), a free optical music
recognition program that runs alongside Registra. Audiveris is AGPL-licensed,
so Registra runs it as a separate program and never builds it in.

## Installing Audiveris

- **Windows / macOS:** install from the Audiveris releases page. Registra
  finds `Audiveris` on the PATH by itself.
- **Raspberry Pi / Linux:** install the Linux package (it needs Java). OMR is
  slow on a Pi 4 and fine on a Pi 5 or a PC.

If Audiveris lives somewhere else, tell Registra how to run it in
`/etc/default/registra` (Pi) or the environment (PC):

```sh
REGISTRA_OMR="/opt/audiveris/bin/Audiveris -batch -export -output {output} {input}"
```

`{input}` is replaced with the scan and `{output}` with an empty folder; any
MusicXML file the program writes there is used. Another OMR engine can be
plugged in the same way.

## Using it

1. **Library → Import**, then choose a PNG, JPEG, TIFF or PDF.
2. Registra reads the music (this can take a minute), then shows the parts.
   The lowest voice is sent to the pedal.
3. Check the speed and the number of verses, then **Import**.
4. Open the track in the editor and look at it in **Score** view. Fix any
   wrong notes in the piano roll before using it in a service.

## Getting good results

- Use clean printed music. Hymn books scan well; handwriting and faint
  photocopies don't.
- Scan flat at 300 dpi, or photograph straight on in good light.
- Do one hymn per image.
