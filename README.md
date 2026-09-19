# Plant Tower Rust

ESP32 firmware for an automated hydroponic tower, written in Rust using the ESP-IDF stack.

## Features

- **Automatic pump control**: pump runs on a configurable on/off interval when enabled
- **MQTT integration**: publishes sensor readings and switch states to Home Assistant via MQTT discovery
- **Temperature sensing**: OneWire DS18B20 sensor
- **Water level monitoring**: digital float sensor
- **RGB LED status indicators**: shows pump state, alerts, and WiFi/MQTT connection state
- **Captive portal**: configure WiFi and MQTT credentials via a browser on first boot or on demand
- **NVS persistence**: configuration survives reboots

## Hardware

| Pin    | Function                               |
|--------|----------------------------------------|
| GPIO13 | Pump relay output                      |
| GPIO18 | Enable pump button (active low)        |
| GPIO19 | Reset connectivity button (active low) |
| GPIO22 | Water level sensor (active low)        |
| GPIO23 | OneWire temperature sensor             |
| GPIO25 | LED channel 0 (PWM)                    |
| GPIO26 | LED channel 1 (PWM)                    |
| GPIO33 | LED channel 2 (PWM)                    |

## Building and Flashing

Requires the Rust ESP-IDF toolchain. Set it up with [esp-idf](https://github.com/esp-rs/esp-idf) prerequisites.

```sh
cargo build --release
cargo espflash flash --release
```

## Configuration

On first boot (or after holding the reset connectivity button for 5 seconds), the device starts a WiFi access point named **Plant Tower Rust**. Connect to it and navigate to the captive portal to enter:

- WiFi SSID and password
- MQTT host, port, username, and password
- MQTT device name and ID

Configuration is stored in NVS and loaded on subsequent boots.

## Project Structure

```
src/
├── main.rs                  # Entry point and main loop
├── captive_portal/          # WiFi AP + HTTP/DNS server for configuration
├── connectivity/            # WiFi client and MQTT connection management
├── controllers/             # Application-level controllers (pump control)
├── hardware/                # GPIO, LEDs, sensors, NVS
├── mqtt/                    # MQTT device, sensors, switches
└── utils/                   # Timer
```
