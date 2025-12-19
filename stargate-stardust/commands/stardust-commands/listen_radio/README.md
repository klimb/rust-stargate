# listen-radio

Play audio from FM radio stations using RTL-SDR (Software Defined Radio).

## Description

The `listen-radio` command allows you to tune into FM radio stations using an RTL-SDR dongle. It demodulates the FM signal and pipes it to an audio player (sox or alsa) for real-time playback.

## Prerequisites

### Hardware
- RTL-SDR dongle (such as RTL2832U-based USB device)

### Software
- **rtl-sdr**: RTL-SDR software package
  - Linux: `sudo apt-get install rtl-sdr`
  - macOS: `brew install librtlsdr`
- **Audio player** (one of):
  - sox (recommended): 
    - Linux: `sudo apt-get install sox`
    - macOS: `brew install sox`
  - aplay (usually pre-installed on Linux)

## Usage

```bash
# Basic usage - tune to 91.3 FM
listen-radio 91.3

# Tune to a specific frequency with PPM correction
listen-radio 98.5 --ppm 32

# Adjust squelch to filter out noise
listen-radio 91.3 --squelch 20

# Set manual gain instead of auto
listen-radio 91.3 --gain 40

# Get JSON output instead of playing audio
listen-radio 91.3 --obj
```

## Arguments

- `<FREQUENCY>`: FM frequency in MHz (e.g., 91.3) or Hz (e.g., 91300000)

## Options

- `-p, --ppm <PPM>`: PPM frequency correction (default: 0)
  - Use this if your RTL-SDR dongle has frequency drift
  - Typical values range from -50 to +50
  
- `-s, --squelch <LEVEL>`: Squelch level (0-100, default: 0)
  - Higher values cut out weaker signals and reduce noise
  - Recommended values: 0-30 depending on signal strength
  
- `-g, --gain <GAIN>`: Tuner gain (default: auto)
  - Can be "auto" or a value from 0-50
  - Auto-gain is usually best for FM radio
  
- `--obj`: Output as JSON instead of playing audio
- `--verbose-json`: Include additional details in JSON output
- `--pretty`: Pretty-print JSON output

## Examples

### Listen to your local NPR station
```bash
listen-radio 91.3
```

### Listen with PPM correction
If you know your dongle has a frequency offset:
```bash
listen-radio 98.5 --ppm 32
```

### Filter out noise with squelch
For weak stations with static:
```bash
listen-radio 91.3 --squelch 15
```

### Manual gain control
If auto-gain isn't working well:
```bash
listen-radio 91.3 --gain 30
```

## Technical Details

The command uses:
- `rtl_fm` for FM demodulation with wide-band FM (WBFM) mode
- Sample rate: 200 kHz
- Audio resample rate: 48 kHz
- Output format: 16-bit signed PCM, mono

Audio is piped directly from `rtl_fm` to the audio player for minimal latency.

## Troubleshooting

### No audio output
- Check that your RTL-SDR dongle is properly connected
- Ensure sox or aplay is installed
- Try adjusting the squelch level
- Check volume settings on your system

### Frequency drift
- Use the `--ppm` option to correct for your dongle's frequency offset
- You can determine your PPM value using kalibrate-rtl or rtl_test

### Noisy or garbled audio
- Try adjusting the squelch level: `--squelch 20`
- Check if you're tuned to the exact frequency
- Ensure your antenna is properly connected

### Device busy error
- Make sure no other application is using the RTL-SDR dongle
- Try unplugging and replugging the dongle

## Related Commands

- `scan-radio`: Scan for radio transmissions and display signal strengths
- `record-audio`: Record audio from various sources

## Notes

- Press Ctrl+C to stop playback
- FM radio frequencies typically range from 87.5 to 108 MHz
- The command requires root privileges on some systems to access the USB device
