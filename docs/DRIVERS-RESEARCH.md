# Organ MIDI driver research

Research date: November 2026. Scope: how each organ maker, console control system and virtual organ program sends and receives **stops, couplers, pistons, expression and notes** over MIDI, so that Registra can implement drivers.

Conventions used here:
- Byte formats are given **only when quoted from a source** (a manual or open-source code).
- "Unknown" means no public source was found. Fill these gaps by capturing traffic from a real console with a MIDI monitor.
- `n` = MIDI channel nibble (0-based). Channel numbers in prose are 1-based.
- Hex values are written without the `0x` prefix.

---

## 1. Priority table

Prevalence: H = widely installed in its markets, M = significant, L = niche.
Docs (documentation quality): A = byte-level spec published, B = partly documented or third-party only, C = nothing public.

| System | Prevalence | How stops are sent | Combination behaviour | Docs | Driver approach | Priority |
|---|---|---|---|---|---|---|
| **Generic** (CC / NRPN / note / PC / constant SysEx) | covers Ahlborn, Eminent, Gem, Phoenix, Musicom and others | user-learned per stop | depends on organ | n/a | data profile | **1** |
| **Hauptwerk 4.2–9** | H (virtual organs everywhere) | whatever the console sends; recommends NRPN 127/0 on/off. Own recorder SysEx `F0 7D 39 72 …` | its own combination system outputs the resulting stops; the recorder stores stop changes, not pistons | A | data profile, plus custom code for the recorder SysEx (optional) | **1** |
| **Allen** (MDS, Renaissance, Quantum, GeniSys) | H (US; strong in AU) | NRPN per stop (CC99, CC98, CC6 = 127/0; no CC38), keyed by (channel, NRPN); stop numbers shared across models | **only a Program Change is sent; no stop data** | A | data profile (NRPN), plus custom combination emulation | **1** |
| **Johannus / Makin / GOG** (modern) | H (Europe, UK; global) | **toggling** Program Change on ch 12 (newer hardware: ch 16); `F0 00 4A 4F 48 41 53 7F F7` = all stops off | stop changes appear on the MIDI SEQ port (implied); exact format unknown | B | data profile (toggle + reset preamble) | **1** |
| **Rodgers** pre-Infinity (Roland-based) | H (US, UK, AU) | Roland-style DT1 SysEx `F0 41 dd 30 12 01 oo <bitmap…> cs F7`; 7 stops per byte; checksum | sends the resulting stop SysEx | A | custom code (stateful bitfield and checksum), with the map as data | **1** |
| **Viscount** (Physis, Unico, Sonus, Chorum) | H (UK, Italy; growing in AU) | absolute SysEx `F0 31 sn xx yy F7` (s = 0 off, 4 on) | receives PC on ch 16 to recall pistons; whether stop SysEx is sent on a piston press is unverified | A | data profile (SysEx template with slots) | **1** |
| **Peterson** (MIDI Resource System, ICS-4000) | H (US pipe organs) | SysEx bitfield `F0 00 00 57 dev [grp d1 d2 d3 d4]… F7`, with a universal stop map | a general piston sends the resulting stop blocks | A | custom code (bitfield blocks), with the map as data | **2** |
| **Solid State Organ Systems** type 1 | M (US, UK pipe organs) | `F0 2B 01 01 22/23 nn nn F7` (22 = off, 23 = on), per stop | resulting stop moves are recorded | B | data profile | **2** |
| **SSOS** type 2 (`F0 2B 01 02 …`) | M | unknown | unknown | C | capture needed | 3 |
| **Rodgers Infinity / Inspire / Imperial** | M–H | "STOP MAP" mode; format unknown (Hauptwerk has an "Infinity+ bitfield" type) | resulting stops sent (likely) | C | capture needed, then custom code | **2** |
| **Rodgers "Phase II"** (early 1990s) | L (legacy) | unknown | unknown | C | capture needed | 3 |
| **Johannus / Makin pre-2000**, "MIDI programmable" | M (legacy UK/NL) | PC on ch 12; MIDI stops send SysEx code 3F followed by a PC | General Cancel handling differs between the two variants | B | data profile | 2 |
| **Ahlborn / Ahlborn-Galanti** | M (DE, US) | Archive: CC73 = on / CC74 = off, value = stop number, ch 16. Organs: CC70, or `F0 35 a b F7` | unknown | B | data profile | 2 |
| **Content, Wyvern, Eminent, Phoenix, Musicom** | M (NL / UK) | Hauptwerk names CC bitfield, key-aftertouch bitfield and PC schemes for these brands | unknown | C | data profile (bitfield / PC variants) | 2 |
| **Classic Organ Works / Artisan** | L–M (Canada / US) | CMK-2: Ahlborn-style CC73/CC74 mode; other modes unknown | pistons use a separate MIDI scheme from stops | B | data profile | 3 |
| **Walker Technical** | M (US) | unknown | unknown | C | capture needed | 3 |
| **Syndyne** | H (US pipe-organ controls) | fixed channels per division; message type unknown | unknown | C | capture needed | 3 |
| **GrandOrgue** | M (hobbyist, growing) | any learned type; outputs Rodgers SysEx, NRPN, note, CC, PC | its own combinations | A (source code) | data profile | 2 |
| **jOrgan** | L | fully user-programmable | n/a | A | data profile | 3 |
| **Organteq** | L–M (home) | CC / note / PC via Action Mapping; no NRPN/SysEx; **no MIDI output** | not observable | B | generic (record-only not possible) | 3 |
| **Copeman Hart, Ruffatti, Sinua, Opus Two, others** | L | unknown (Sinua at Orgelpark: note-on on ch 14) | unknown | C | capture needed | 3 |

---

## 2. Per-system details

### 2.1 Generic

OrganAssist's "Generic" driver covers Ahlborn, Eminent, Gem, Musicom, Phoenix and "others", with each stop learned individually ([OrganAssist features](https://www.organassist.com/wp/features/)).

Per-stop fields in OrganAssist ([User Manual PDF](https://organassist.com/downloads/UserManual.pdf)):
- **Message fields:** MIDI On, MIDI Off, or MIDI Toggle, with up to 5 alternative messages per stop (NRPN and SysEx can be mixed).
- **`FF` wildcard:** marks the byte that varies.
- **`FD` "double" marker:** used for pistons that send a pair of messages meaning "reverse this stop"; OrganAssist acts on the first message of the pair.
- **CC38:** transmitted after NRPNs by default. On input, NRPNs are accepted with or without it. An ini flag (`DISABLECONTROLLER38=True`) turns it off per organ.
- **Timing:** "Repeat ms" (about 120 ms suggested) spaces out on/off messages sent to moving drawstops, and there is also a "Delay ms".

Example shapes listed on the [OrganAssist history page](https://www.organassist.com/wp/history/):

| Shape | Example |
|---|---|
| Distinct on/off SysEx (Gem) | `F0 35 14 06 F7` = on, `F0 35 1A 06 F7` = off |
| Same message for on and off (toggle) | `CB 1A 00` |
| List of stops followed by an end marker | `CB 22 00 CB 24 00 CB 7F 00` |
| 7-stop bitfield in one CC | `B0 10 63` |
| Whole-organ Roland-format SysEx | `F0 41 10 30 12 01 00 00 04 39 … F7` |

### 2.2 Hauptwerk (4.2 → 9)

Source: Hauptwerk v9 Advanced User Guide, reference section pp. 121–128 ([PDF](http://downloadhauptwerk.com/documents/hw9/HauptwerAdvancedUserGuide.pdf), linked from [hauptwerk.com/documentation](https://www.hauptwerk.com/documentation/)).

**Switch input types.** These show which manufacturer schemes exist in the field. The guide gives no byte layouts for the manufacturer-specific types.
- **Momentary pistons:**
  - Program Change.
  - Note-on.
  - NRPN-on (127).
  - CC-on (127).
  - Johannus 9-byte and 11-byte SysEx.
  - Hoffrichter types.
  - Alternating note on/off.
  - Alternating CC 127/0.
- **Stops or hold-pistons:**
  - NRPN 127/0 (the guide cites Allen as an example).
  - Note on/off.
  - CC 127/0.
- **Manufacturer stop types:**
  - Rodgers pre-Infinity bitfield.
  - Rodgers Infinity+ bitfield (speaking stops and other controls).
  - Johannus/Makin/Wyvern/Musicom toggling Program Changes.
  - Content/Wyvern/Eminent CC bitfield.
  - Content/Wyvern key-aftertouch bitfield.
  - Viscount 6-byte constant SysEx.
  - Ahlborn Organum/Praeludium varying NRPNs.
  - Ahlborn CC70.
  - Ahlborn Archive CC73/74.
  - Makin Program Change sequences.
  - Phoenix/Wyvern Program Changes, where "off" uses the same program number as "on" but on the next MIDI channel.
- **Other:**
  - Generic PC, CC, RPN and NRPN.
  - Constant SysEx up to 8 bytes.
  - Key aftertouch.
  - "Notation" CC ranges.

**Continuous controls:**
- CC, RPN and NRPN, including 14-bit values.
- Aftertouch.
- Ahlborn crescendo sent as Program Change ranges (64h–6Dh, 64h–6Fh or 64h–7Ch, for 9, 11 or 24 stages).

**MIDI output:**
- Switch output echoes the received message type.
- If output must differ from input, only NRPN 127/0 or note on/off can be used.
- Hauptwerk recommends NRPN 127/0 for new consoles.

**Combinations:**
- Use either the organ's combination system (learn only the stops) or Hauptwerk's (learn only the pistons), never both, or the two "fight".
- The Hauptwerk recorder records the resulting stop changes, not combination events.

**Recorder SysEx (p. 127):** `F0 7D 39 72 cc oo oo oo oo nn vv [lsb] F7`.
- `cc` values: 16 = switch on, 17 = switch off, 18 = continuous control, 19/20 = key on/off, 21/22 = second touch, 23/24 = rank note, 28 = file start, 29 = file end.
- `oo oo oo oo` = 28-bit object ID.
- NRPN, note and CC messages are used in preference to this SysEx.
- Recorded files are tied to one organ (sample set).

**LCD and status SysEx:**
- LCD: `F0 7D 01 idLSB idMSB colour <32 ASCII> F7`.
- Status: `F0 7D 1B var val F7`, `F0 7D 1A var b1..b4 F7`, `F0 7D 19 var <16 chars> F7`.

**Version changes** ([v9 Release Notice](http://downloadhauptwerk.com/documents/hw9/HauptwerkReleaseNotice.pdf)):
- v5: "sequencer" ports renamed to "MIDI recorder/player"; Musicom added to the toggling Program Change type.
- v6: 32 console MIDI ports.
- v7: per-organ and global MIDI assignments.
- v8–v9: UI changes only.
- No new switch types were found after v5. The v4.2 type list was not checked.

### 2.3 Allen

**Stops and couplers** ([TH317e manual, appendix "Advanced MIDI Information – NRPN Stop Data"](https://www.allenorgan.com/support/ownersmanuals/033-00266TH317eTheatreOwnersManual.pdf)):
- Each stop is one NRPN on its division's channel: `Bn 63 msb`, `Bn 62 lsb`, `Bn 06 7F` (on) or `Bn 06 00` (off).
- Example: Tibia Clausa 8' = NRPN 40 on channel 1. Stop 132 = MSB 1, LSB 4.
- Allen keeps one stop-number map across models, so the same voice has the same NRPN on any Allen.
- Couplers are NRPNs too, e.g. Accomp. to Pedal = channel 3, NRPN 248. GeniSys Voices 1–10 = NRPN 501–510.
- The same NRPN number is reused in different divisions. On the MDS-39, NRPN 40 is Gedackt 8 on Swell (ch 1) and Quintadena 8 on Positiv (ch 4) ([MDS-39 NRPN list](https://users.ox.ac.uk/~newc0543/music/Allen%20MDS-39%20NRPN%20list.pdf)). **The driver must key stops on (channel, NRPN).**
- No CC38 (data entry LSB) is sent ([Modartt forum](https://forum.modartt.com/viewtopic.php?id=12492)).
- The console setting "Transmit/Receive Stops" turns stop NRPNs on or off ([Chamber Series Console Controller guide](https://www.allenorgan.com/support/ownersmanuals/033-00086ChamberConsoleController.pdf), §C-11).

**Pistons** (same Chamber guide):
- Pistons send Program Change on the division's channel. Generals go on channel 8, and G1/G2 pistons on channel 7.
- Classical channel layout: Swell 1, Great 2, Pedal 3, Choir 4, Solo 5.
- The base channel shifts all channels and resets to 1 at power-on.
- On the wire, Program Change = piston number − 1 (TH317e).
- Program Change modes: PRESET, USER (with bank select) and NO.
- In PRESET mode, GeniSys also sends the capture memory level when it changes. The message type for this is unknown ([GeniSys Overview](https://allenorgan.com/wp-content/uploads/2025/09/033-00225-3B_GeniSys_Overview-3.pdf)).

**Combination quirk:**
- Piston changes are always sent as Program Changes, even when stop data is turned off (Chamber guide §C-11).
- The explicit statement that Allen sends no stop data when a combination is used comes from OrganAssist ([Allen page](https://www.organassist.com/wp/allen-organs/); also quoted at [tech.churchofjesuschrist.org](https://tech.churchofjesuschrist.org/forum/viewtopic.php?p=252059)). That is not an independent source.
- OrganAssist offers three modes: stops only; record pistons as Program Changes; or OrganAssist's own internal combinations, which send every stop change itself.

**Expression:** CC7 on each division's channel ("Poly-Volume", the default), with a minimum value of about 20. Other modes: velocity-based, Swell-only, or none (Chamber guide §C-7).

**Notes:** Note-off is sent as Note On with velocity 0.

**Capture memory:** can be dumped to and from a sequencer as SysEx, but the format is not published.

**Quantum and GeniSys:** no change to the NRPN scheme was found.

### 2.4 Rodgers

**Pre-Infinity models** ([Rodgers 690 manual pp. 45–47](https://www.manualslib.com/manual/902957/Rodgers-690.html?page=45); manualslib OCR, so verify against the original scan):
- **Message structure:** `F0 41 <dev> 30 12 <sub> <offset> <data…> <cs> F7`. 41 is the Roland manufacturer ID, 30 is the organ model ID and 12 is "Data Set".
- **Sub-command 01 (Stop Change):** sent whenever a stop, coupler or auxiliary control changes.
  - It is a bitmap with one bit per control (1 = on), normally the full 35 bytes.
  - The offset (00–22h) gives the byte where the bitmap starts.
  - The manual says bit assignments are standardised across Rodgers organs. Example: Great 8' Principal = byte 0, bit 3.
- **Sub-command 03:** proprietary memory dump.
- **Checksum:** Roland style; the low 7 bits of the sum from the sub-command through the checksum must be 0.
- **Device ID:** 10h by default on the 690. The Masterpiece manual shows "17", which is probably the 1-based display of 10h (inference) ([Masterpiece manual](https://www.rodgersinstruments.com/uploads/products/pdfs/om-pdfs/1905865.pdf)).

**GrandOrgue's implementation** confirms the frame ([GOMidiEvent.cpp](https://github.com/GrandOrgue/grandorgue/blob/master/src/grandorgue/midi/events/GOMidiEvent.cpp), [GORodgers.cpp](https://github.com/GrandOrgue/grandorgue/blob/master/src/grandorgue/midi/events/GORodgers.cpp)):
- Header check: `F0 41 dev 30 12 01 key data… cs F7`.
- Checksum computed over `01 key data…`.
- Stop n = bit (n mod 7) of byte (n div 7).
- When sending, GrandOrgue keeps per-device state and sends only the changed byte (using the offset).
- OrganAssist likewise stores a byte/bit position per stop (e.g. "1A 40") and builds the long SysEx from the state of every stop ([User Manual](https://organassist.com/downloads/UserManual.pdf)).

**Stop Change send/receive setting** ([835 manual p. 40](https://www.manualslib.com/manual/1783463/Rodgers-835.html?page=40)):
- Options: OFF, STP (stops), PST (pistons) or S&P (both).
- Piston Program Changes: 1–12 = generals, 20 = general cancel, 21–26 = divisionals (Masterpiece manual).

**Channels:** master channels Great 12, Swell 13, Pedal 14, Choir 15, Solo 16. The CC used for expression is unknown (the implementation chart lists CC7 and CC64).

**Combinations:** Rodgers sends the resulting stop SysEx: "Combiners, Tutti and Crescendo send SysEx strings listing their related stops" ([organforum](https://organforum.com/forums/forum/organ-building-repair-restoration/virtual-organs/852770-rodgers-issue-with-hauptwerk)).

**Snapshot:** the 690 sends its current stop and expression state when it receives MIDI Start. Useful at the start of a recording.

**Infinity:** the "TX Piston" setting is OFF, STOP MAP (detailed stop data, which can delay the first note) or PC (bank + piston). The byte format is unknown ([Infinity guide](https://www.rodgersinstruments.com/uploads/owners_manual/owners_guides_2017/1905371_H00_INFINITY_ORGAN.pdf)). Hauptwerk's separate "Infinity+ bitfield" type implies a different layout.

**Phase II, Inspire, Imperial:** unknown.

### 2.5 Johannus / Makin / Global Organ Group (GOG)

**Platform:** current Makin organs use the same platform as Johannus. The MIDI chapter of the Makin 2023 manual is identical to the Johannus one ([Makin manual](https://www.makinorgans.co.uk/uploads/owners_manual/Makin_User_Manual_2023.pdf)). GOG owns Johannus, Makin, Copeman Hart and Rodgers ([Rodgers: what is GOG](https://www.rodgersinstruments.com/news/what-is-the-global-organ-group)). It is unconfirmed whether Copeman Hart uses the Johannus scheme.

**Ports** ([Ecclesia D47 manual](https://hq.globalorgangroup.com/service/scripts/customer.fcgi/getAttachment/39921-P4jPYl834UzpHIrW4wcCJISSZG0zzT2Hnz4hRSZSWf3ByGy9Dt3YHxN2GygSLO5W-0/Ecclesia%20D47%20usermanual.pdf)):
- MIDI IN.
- MIDI MOD: programmable, meant for a sound module.
- MIDI SEQ: fixed, meant for a sequencer or PC. **Record from SEQ.**

**Stops:**
- Sent and received as Program Change on channel 12. Manuals are on channels 1–5.
- Each Program Change **toggles** a stop; it carries no on/off state ([zerkelhome guide](https://www.zerkelhome.com/organ/), [organforum](https://organforum.com/forums/forum/electronic-organs-midi/classic-church-electronic-organs/822868-need-information-on-using-midi)).
- Program Change numbers depend on the model. Example from one user's Opus 10: Pedal 0–8, Swell 10–19, Great 21–30 ([magle.dk](https://www.magle.dk/music-forums/threads/johannus-opus-10-to-talk-to-hauptwerk.10750/)).
- On LiVE III (GOG hardware V3), the accessories channel is 16 and divisions are on channels 7–10 ([LiVE III manual](https://hq.globalorgangroup.com/service/scripts/customer.fcgi/getAttachment/42266-fa2Dru2YFcuq7uAiLAIKrFhNtM1MlFqocmzh3F9cEyNOUWWzLBQOyoqnp2ksWCzI-0/LiVE%20III%20EN%20Usermanual.pdf)).

**SysEx:** `F0 00 4A 4F 48 41 53 XX F7` ("JOHAS").
- `XX` = 7F means all stops off. It is sent on SEQ when the 0/Cancel piston is held, and received to clear the stops.
- A short press of 0 also sends all volume settings.
- GrandOrgue decodes this 9-byte form (the stop toggles when two identical messages arrive in succession) and an 11-byte form, `F0 00 4A 4F 48 41 53 vv 10 kk F7` ([GOMidiEvent.cpp](https://github.com/GrandOrgue/grandorgue/blob/master/src/grandorgue/midi/events/GOMidiEvent.cpp)).

**Verified from the Makin manual** (Rydal, Thirlmere, Windermere and Derwent; implementation card dated September 2008; [Makin manual](https://www.makinorgans.co.uk/uploads/owners_manual/Makin_User_Manual_2023.pdf) pp. 17, 38–40):
- Default channels, sent and received:
  - Rydal and Thirlmere: Great 1, Swell 2, Pedal 3, stops 12.
  - Windermere: Choir 1, Great 2, Swell 3, Pedal 4, stops 12.
  - Derwent: Choir 1, Great 2, Swell 3, Solo 4, Pedal 5, stops 12.
- Program Change numbers depend on the number and order of stops on each model, so they must be learned. MIDI stops (programmable) use 1–128.
- Controllers sent:
  - CC7 general volume: 40–127.
  - CC11 swell pedal: 55–127.
  - Pitch: RPN-style CC101=0, CC100=1, then CC6 data 33–95, where 64 = A440.
  - Transposer: CC100=2, then CC6 data 56–72, where 64 = no transposition.
  - The channel for CC7 and CC11 is not stated.
- Controllers received: CC7 and CC11 accept 0–127, but values below 40 and 55 respectively are treated as 40 and 55.
- Every SysEx has the form `F0 00 4A 4F 48 41 53 XX F7`.
  - `7F` = all stops off. It is sent on SEQ when Cancel (CAN) is pressed; receiving it clears the stops.
  - Pressing Cancel also sends all volume settings on SEQ.
  - Piston "pushbutton" values are sent on MOD only, and matter only for the CSM 128 sound module.
- Sockets: MIDI IN, MIDI MOD (programmable, for a sound module) and MIDI SEQ (not programmable, for a sequencer or PC).
- Registra ships this as the built-in **Makin 2-manual (template)** (`organs/makin-2m.organ.json`). Its channels and messages follow the manual; its stop list and Program Change numbers are placeholders until learned from a real console.

**Older "MIDI programmable" models:** switching a MIDI stop sends SysEx code 3F followed by a Program Change on SEQ ([Vivaldi 15 manual](https://manualshive.com/en/manual/johannus/vivaldi-15/2021450?page=33)).

**Pistons:**
- A piston press sends a SysEx "pushbutton" value on MOD only.
- The GOG Sequencer v2 manual implies that preset-driven stop changes do reach SEQ ([Seq v2 manual](https://hq.globalorgangroup.com/service/scripts/customer.fcgi/getAttachment/46806-DDzRK6dw9mOQ1D301qIFRH00ZAZDln3axOgMgQZcZoNq55SUcifEECu2mm9HPhaM-0/MIDI%20Sequencer%20v2%20Usermanual.pdf)). The exact format is unknown.

**Expression:** CC11 on each division's channel (range 55–127; 63–127 on the D47). General volume is CC7.

**Tremulant:** an ordinary stop Program Change. No speed or depth data.

**Playback rule:** send all-stops-off first, wait a short time ("10 ticks", per zerkelhome), then replay the toggles. The driver must track stop state itself.

### 2.6 Viscount (Physis, Unico, Sonus, Chorum)

Source: [Physis Reference Manual v1.17 §14.6](https://www.viscountinstruments.com/img/PhysisEU_ReferenceManual-EN-v117-7.pdf); the [Chorum manual §10.6](https://chci-varhany.cz/wp-content/uploads/2021/03/Chorum-EU-EN-v14-1.pdf) uses the same scheme.

**Channels:**
- Sections on channels 1–5, with the top manual first.
- Channel 16 is reserved for system messages.

**Stops:** `F0 31 sn xx yy F7`.
- 31 = Viscount ID.
- `s` = 0 (off) or 4 (on).
- `n` = section channel.
- `xx yy` = voice ID. It is not listed in the manual; read it from the Physis Editor or capture it.
- GrandOrgue decodes this as a 6-byte frame with value `(b2<<14)|(b3<<7)|b4` ([GOMidiEvent.cpp](https://github.com/GrandOrgue/grandorgue/blob/master/src/grandorgue/midi/events/GOMidiEvent.cpp)).

**Couplers and cancels:**
- Couplers: `F0 31 2F 70 vv F7`. The manual marks these "received only", although the sequencer records couplers.
- Cancel: `F0 31 2F 6C 00 F7`.
- Reed cancel: `F0 31 2F 6A vv F7`.
- Mixture cancel: `F0 31 2F 6B vv F7`.

**Pistons (received):** Program Change on channel 16.
- 1–10 = generals.
- 17–22 = Pedal combinations.
- 20h–56h = divisional combinations.
- 96/97 = Tutti off/on.
- 112–127 = Crescendo steps.
- Not verified: whether a piston press transmits each stop's SysEx.

**Tremulant (the known quirk):**
- Switching a tremulant on transmits its speed and depth; switching it off sends depth 00.
- 1st tremulant: speed `F0 31 2s 5A vv F7`, depth `…5C…`.
- 2nd tremulant: speed `…5B…`, depth `…5D…`.
- `vv` range: 04–20h.

**Expression:** CC11 on each section channel; divisional volume is CC7.

**Active Sensing:** sent every 300 ms (filter it out).

**Older or cheaper models** (e.g. C300, Sonus 40) reportedly send keys only, with no stops ([viscountorgans.net](https://viscountorgans.net/viscount-organs-forum/sonus-40-midi-control)).

### 2.7 Peterson (MIDI Resource System, ICS-4000)

Source: [MIDI User Instructions (implementation chart V1.68)](https://www.petersonemp.com/manuals/manuals/MIDI%20Resource%20System/MIDI%20User%20Instructions.pdf).

**Stops:** SysEx `F0 00 00 57 <dev>` followed by one or more 5-byte blocks.
- 00 00 57 is Peterson's manufacturer ID.
- Each block is a group byte `0ssssmmm` (ssss = sub-group, mmm = division) plus 4 data bytes of 7 stop bits each, i.e. 28 stops per block.
- Blocks can be chained and cut short with `F7`. Omitted bytes mean "no change".
- Bits map to the printed "Peterson Universal Stop List".
- Example: Great 4' Octave on = `F0 00 00 57 01 01 01 00 00 00 F7`, off = `… 01 01 00 00 00 00 F7`.
- The document contains typos, so verify against captures.
- The system may break a long stop message to insert key data, then start a new header. The parser must tolerate this.

**Couplers:** bits in the same map.

**Pistons:**
- A general piston sends the resulting stop blocks.
- "Auto Resend" re-sends the full state periodically, so playback can start mid-file.

**Expression:** CC7 on each division's channel.

**Channels:**
- Swell 1, Great 2, Choir 3, Pedal 4, Solo 5.
- Notes 24–108, with note-off sent as velocity 0.
- The receiver also recognises the Allen ID `00 00 35`.
- The "Instrument Out" port strips SysEx; record from "Sequencer Out".

**ICS-4000:** uses the same proprietary stop encoding ([ICS-4000 manual 2024](https://www.petersonemp.com/manuals/manuals/ICS-4000/ICS%20Users%20Instructions/ICS4000%20User%20Manual%2001-10-2024.pdf)). The 1,000th ICS-4000 shipped in 2016, chosen by 220 organ firms ([The Diapason](https://www.thediapason.com/news/peterson-one-thousandth-ics-4000%E2%84%A2)).

**"Peterson SysEx swell":** OrganAssist supports it from v3.2.26 ([User Manual](https://organassist.com/downloads/UserManual.pdf)), but the byte format is **unknown**. The Peterson document itself only shows CC7 expression.

**Duet and MSP-1000:** no stop MIDI format documented.

### 2.8 Solid State Organ Systems (SSOS)

**Manufacturer ID:** 2B is registered to "Solid State Logic Organ Systems" ([Wireshark ID table](https://www.wireshark.org/docs/wsar_html/data-midi-sysex-id_8h_source.html)).

**Type 1 (MultiSystem II)** ([MIDI Operation p. 8](https://ssosystems.com/downloads/MultiSystem%20II%20MIDI%20Operation.pdf)):
- The manual says: "Stops are transmitted/received as 8 byte MIDI SysEx messages. `F0-2B-01-01-22/23-nn-nn-F7`; 22 = Stop Off, 23 = Stop On, nn-nn = internal stop number."
- The stop number is organ-specific, so a learn step is needed.
- Expression is sent as "MIDI Master Volume". Whether that means CC7 or Universal SysEx Master Volume is ambiguous.
- Notes are sent with velocity 64.
- Channels: Great 1, Swell 2, Choir 3, Pedal 4.
- Stops are only sent on OUT2.

**Type 2 (`F0 2B 01 02`, "circa 2010"):** unknown. OrganAssist says only that it uses "longer MIDI codes" ([User Manual](https://organassist.com/downloads/UserManual.pdf)).

**MFM-D** ([Operation Guide](https://ssosystems.com/downloads/MFM-D%20Operation%20Guide.PDF)):
- "Sys Ex Transmit" options: Standard Sequence, or Stop Move Sequence (the latter is for the CFM combination action and is "currently not used").
- MIDI stops can send CC7 or CC11.

**Playback warning (from the manual):** use General Cancel before and after recording; stopping playback early leaves stops on.

### 2.9 Ahlborn / Ahlborn-Galanti

**Archive modules** ([CMK-2 manual p. 51](https://manualslib.com/manual/993008/Classic-Organ-Works-Cmk-2.html?page=51)):
- Stop on = `Bn 49 xx` (CC73), stop off = `Bn 4A xx` (CC74), where `xx` = stop code, on channel 16.
- `xx` = 7F means all stops.
- Tremulant: CC92 (`5C`), value 7F/00.
- Presets are recalled by Program Change on channel 16 ([Peterson note on Archive modules](https://www.petersonemp.com/manuals/manuals/ICS-4000/Using%20an%20Ahlborn%20Archive%20Module%20with%20your%20ICS-4000.pdf)).

**Organs:**
- Hauptwerk lists "Ahlborn CC70" and "Organum/Praeludium varying NRPNs" types.
- GrandOrgue decodes `F0 35 a b F7` (value `(a<<7)|b`), with one value for off and one for on. 35 is the Generalmusic (Gem/Galanti) ID; this matches OrganAssist's Gem example.
- GrandOrgue also has a "CC fixed value" type aimed at Ahlborn (on = value 64–127, off = value − 64).

**Pistons and expression:** unknown.

### 2.10 Content, Wyvern, Eminent, Phoenix, Musicom, Copeman Hart

- **No public MIDI documentation was found** for any of these.
- The only technical evidence is the set of Hauptwerk switch types named after them (see §2.2): CC bitfield (Content/Wyvern/Eminent), key-aftertouch bitfield (Content/Wyvern), toggling Program Changes (Wyvern/Musicom), and Program Change on/off split across adjacent channels (Phoenix/Wyvern).
- GrandOrgue's CC bitfield type is described in its code as "for some Content organs", with up to 7 stops per CC value.
- Content's current Cambiare MIDI consoles are Hauptwerk/Sweelinq front-ends ([Content](https://www.contentorgans.com/en/cambiare-organs/cambiare-midi-console/)).
- OrganAssist lists Content and Wyvern drivers as "tested / should work" ([User Manual](https://organassist.com/downloads/UserManual.pdf)).

### 2.11 Classic Organ Works / Artisan

- Classic Organ Works is a division of Artisan Classic Organ Inc. (Markham, Ontario). Its Maestro system is a Linux PC with USB stop boards ([organworks.com](https://organworks.com/index.php/articles/328-cost-effective-maestro-system)).
- **CMK-2 controller** ([manual](https://www.manualslib.com/manual/993008/Classic-Organ-Works-Cmk-2.html?page=44)):
  - SysEx ID `00 20 1C`.
  - Supports the universal Identity Request.
  - Ahlborn mode uses CC73/CC74 as in §2.9.
  - Hauptwerk mode sends pistons as Program Change 1–128.
  - Stop encoding in Hauptwerk mode: unknown.
- OrganAssist notes that pistons use a different MIDI scheme from stops, and that each stop must be learned through a one-stop general piston ([User Manual](https://organassist.com/downloads/UserManual.pdf)).

### 2.12 Walker Technical, Syndyne, Ruffatti and other pipe-organ controls

- **Walker:** nothing public was found.
- **Syndyne LS2400K** ([manual p. 9](https://syndyne.com/support/2400manual.pdf)): "All organ functions are automatically transmitted" on fixed channels: Pedal keys and stops on ch 2, Great ch 3, Swell ch 4, and so on, with accessories on ch 9. The message type used for stops is unknown.
- **Syndyne MS8400:** supports record/playback, but Syndyne "does not offer solutions" for third-party MIDI use ([MS8400 manual](https://syndyne.com/support/MS8400BuilderManualWeb.pdf)). A builder describes Syndyne as used by "the majority of organ builders" ([Leek Pipe Organs](https://leekpipeorgans.com/2024/03/21/keeping-pipe-organ-control-systems-aka-brains-of-the-organ-current)).
- **Sinua** (one console at Orgelpark Amsterdam): each register is a Note On on channel 14 (note = register − 1), and combination buttons are notes on channel 15 ([interface manual](https://www.orgelpark.nl/content/files/instrumenten/interface-manual-v4.pdf)). This is probably specific to that installation.
- **Ruffatti, Opus Two, Heuss, Laukhuff, Matters, OSL:** unknown.

### 2.13 GrandOrgue

Source code: [`src/grandorgue/midi/`](https://github.com/GrandOrgue/grandorgue/tree/master/src/grandorgue/midi). The project is GPL; it is a good reference for decoders written by people with real consoles.

**Receive types:**
- Note variants and short-octave.
- CC: on/off/fixed and CC bitfield.
- Program Change and Program Change range.
- RPN and NRPN.
- SysEx: Johannus 9-byte and 11-byte, Viscount (plus toggle), Rodgers stop change, Ahlborn-Galanti (plus toggle).

**Send types:**
- Note, CC, RPN, NRPN, Program Change.
- Rodgers stop change.
- Hauptwerk-compatible LCD and name SysEx.

**Recorder:** GrandOrgue has its own recorder SysEx, `F0 7D 47 4F …`.

### 2.14 jOrgan, Organteq, Aeolus

- **jOrgan:** byte matching is fully programmable via its MPL language. Switches have Activate, Deactivate and Toggle inputs ([jorgan.info](https://jorgan.info/base/Midi.html)).
- **Organteq:**
  - Stops are mapped through "Action Mapping" (CC, note, Program Change).
  - No NRPN or SysEx support ([forum 12492](https://forum.modartt.com/viewtopic.php?id=12492)).
  - No MIDI output, so Registra cannot read stop state from it ([forum 11296](https://forum.modartt.com/viewtopic.php?id=11296)).
- **Aeolus:** uses a single CC (#98 by default) that encodes group and mode (off/on/toggle) and then a button number (Aeolus README, in the aeolus-0.10.4 source package from kokkinizita.linuxaudio.org).

---

## 3. Prevalence and prioritisation

There is no hard market-share data. These are the indicators found:

| Region | Indicator | Source |
|---|---|---|
| US | Allen says it is the largest supplier to the US institutional market (SEC 10-K) | [10-K 2004](https://www.sec.gov/Archives/edgar/data/0000003753/000000375305000004/n10k2004.txt) |
| US | Rodgers is a major US brand, owned by GOG since 2016 | [Rodgers](https://www.rodgersinstruments.com/news/what-is-the-global-organ-group) |
| US pipe organs | Peterson ICS-4000: 1,000+ systems, 220 firms. Syndyne described as used by "the majority of organ builders". SSOS has offices in Virginia and Suffolk (UK) | [Diapason](https://www.thediapason.com/news/peterson-one-thousandth-ics-4000%E2%84%A2), [Leek](https://leekpipeorgans.com/2024/03/21/keeping-pipe-organ-control-systems-aka-brains-of-the-organ-current) |
| Europe / global | GOG (Johannus) calls itself "the largest player in the digital organ world", exporting to more than 100 countries | [GOG](https://www.globalorgangroup.com), [Wikipedia](https://en.wikipedia.org/wiki/Johannus) |
| UK | Makin is "one of the strongest brands in the UK" (GOG). Viscount, Copeman Hart and Wyvern are UK-voiced brands | [GOG](https://www.globalorgangroup.com) |
| Australia / NZ | Allen has been installed in WA churches since 1974 (70+ organs). Rodgers Infinity at St Mary's Cathedral, Sydney. Johannus has a NSW presence. Viscount distribution in AU since 2019 | [Allen AU](https://www.allenorgan.com.au/history.shtml), [Rodgers](https://www.rodgersinstruments.com/news/st-maryrs-cathedral-sydney-australia-celebrate-their-new-rodgers-infinity-243mv), [viscountorgans.net](https://viscountorgans.net/australia/) |
| Virtual | Hauptwerk is the de facto standard. GrandOrgue: about 6,000 downloads of release 3.17.3 (GitHub API) | [GitHub](https://github.com/GrandOrgue/grandorgue) |

**Suggested order:**
1. **P1:** Generic, Hauptwerk, Allen, Johannus/Makin, Rodgers pre-Infinity, Viscount.
2. **P2:** Peterson, SSOS type 1, Ahlborn, the bitfield family (Content/Wyvern/Eminent/Phoenix), GrandOrgue, Rodgers Infinity (needs a capture).
3. **P3:** everything that is undocumented (SSOS type 2, Walker, Syndyne, Classic Organ Works, Copeman Hart, Rodgers Phase II).

---

## 4. Driver architecture implications

A data-driven profile format should support the following. Items marked † are better done as small code plugins that the profile references.

1. **Message templates with slots.** Fixed bytes plus named slots (`{stop}`, `{ch}`, `{value}`, `{msb}`, `{lsb}`), with masks and shifts. This covers OrganAssist's FF/FD/FE wildcards and calculation sets. Examples: Viscount `F0 31 {s<<4|ch} {id_hi} {id_lo} F7`, SSOS `F0 2B 01 01 {22|23} {nn} {nn} F7`.
2. **Multi-message sequences.**
   - NRPN as CC99, CC98, CC6, with optional CC38. CC38 must be configurable on output (Allen sends none) and optional on input.
   - Running status.
   - Messages to other devices interleaved within a sequence.
3. **Stop identity keyed by (channel, number)**, not by number alone (Allen reuses NRPN numbers across divisions).
4. **Stop semantics:**
   - Absolute on/off: separate on and off messages, or one message carrying a value.
   - **Toggle** (Johannus Program Change, Johannus 9-byte SysEx, OrganAssist "Midi Toggle"). This needs the driver to track state and send a **reset preamble** (all-off SysEx) plus a settle delay before playback.
   - "Double/reverse" message pairs.
   - On/off split across adjacent channels (Phoenix/Wyvern).
   - Value ranges (Ahlborn on = value 64–127, off = value − 64).
5. **Bitfield and state-image encodings:**
   - 7 stops per data byte, as a CC value (Content), aftertouch (Content/Wyvern), or SysEx (Rodgers, Peterson).
   - Byte/bit addressing per stop (OrganAssist's "1A 40").
   - Decoding must diff against the previous state. Encoding must either rebuild the full image or send only the changed bytes with an offset (Rodgers).
   - Partial and "no change" blocks (Peterson).
   - † Peterson group-block chaining and interrupted messages.
6. **Checksums:** † Roland/Rodgers (two's complement of the 7-bit sum over a byte range, configurable). Leave room for other checksum types.
7. **Combination handling flags per profile:**
   - `pistonSendsStops` (Rodgers, Peterson, probably Johannus): record the stops and ignore pistons.
   - `pistonWithoutStopFeedback` (Allen): record the piston Program Change. During playback either send the Program Change and trust the organ's capture memory (fragile if memories change), or emulate combinations by storing the registration per piston and memory level and sending stop messages. Memory-level messages are needed (format unknown for Allen).
   - Piston receive maps (Viscount PC on ch 16; Rodgers PC 1–26).
8. **Snapshot / initial state:** request or send the full state at record start (Rodgers responds to MIDI Start; Peterson Auto Resend; Johannus all-off plus volume dump). Store the initial registration in each recording.
9. **Expression mappings per division:** CC7 or CC11 (different makers differ), velocity-mode warnings, clamp ranges (Johannus 55–127, Allen minimum about 20), 14-bit (MSB/LSB on separate channels, Syndyne), and slots for unknown SysEx swell types (Peterson).
10. **Extra parameters on stops:** tremulant speed and depth carried with the tremulant state (Viscount). Model these as stop "attributes" stored with the event.
11. **Port roles:** many organs send stops only on a specific port (Johannus SEQ, SSOS OUT2, Peterson Sequencer Out). The profile should state which port to use, so the UI can warn the user.
12. **Filters:** Active Sensing (Viscount 300 ms), MIDI clock, and SysEx this driver does not handle.
13. **Timing controls:** inter-message delay and repeat time for moving drawstops (OrganAssist suggests about 120 ms), and throttling of large state images (Peterson about 15 blocks per general piston).
14. **Learn mode for opaque IDs:** Viscount voice IDs, SSOS internal numbers and Johannus Program Change numbers are organ-specific. The profile should hold the message *shape*, and a per-organ map holds the numbers, captured via MIDI learn.
15. **Universal stop maps as data:** Allen NRPN map, Peterson Universal Stop List, Rodgers standard bit map. Use these for cross-organ "equivalents" (as OrganAssist does with Division / Pitch / Category / Character).
16. **Vendor recorder formats:** † Hauptwerk `F0 7D 39 72` and GrandOrgue `F0 7D 47 4F`. Recognise and preserve these, or convert them when importing MIDI files.
17. **Capture tooling:** P3 systems need a "capture session" feature (raw log plus annotation such as "pressed Gt Principal 8 on") so users can contribute profiles.
