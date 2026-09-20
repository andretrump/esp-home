# esp-home

Rust monorepo for ESP32 home automation firmware, built on the ESP-IDF stack.

## esp-home-kit

Reusable library for ESP32 devices that integrate with Home Assistant over MQTT.

- **Captive portal**: configure WiFi and MQTT credentials via a browser on first boot or on demand
- **WiFi + MQTT connection management**: automatic reconnection, message queuing
- **MQTT discovery**: sensors and switches with Home Assistant auto-discovery
- **Hardware abstractions**: GPIO, PWM LEDs, OneWire temperature sensor, NVS storage
- **Timer utility**: elapsed-time based scheduling

### Structure

```
esp-home-kit/                # Reusable library for ESP32 home automation
└── src/
    ├── captive_portal/      # WiFi AP + HTTP/DNS server for configuration
    ├── connectivity/        # WiFi client and MQTT connection management
    ├── hardware/            # GPIO, LEDs, sensors, NVS
    ├── mqtt/                # MQTT device, sensors, switches
    └── utils/               # Timer
```

## plant-tower

ESP32 firmware for an automated hydroponic tower.

- **Automatic pump control**: pump runs on a timed on/off cycle; off duration shortens at higher temperatures
- **MQTT integration**: publishes sensor readings and switch states; supports remote enable/disable
- **Temperature sensing**: OneWire DS18B20 sensor
- **Water level monitoring**: digital float sensor
- **RGB LED status indicators**: shows pump state, alerts, and WiFi/MQTT connection state

### Structure

```
plant-tower/                 # Application binary for the hydroponic tower
└── src/
    ├── main.rs              # Entry point and main loop
    └── pump_controller.rs   # Pump control logic
```

### Hardware

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

### Configuration

On first boot (or after holding the reset connectivity button for 5 seconds), the device starts a WiFi access point named **Plant Tower Rust**. Connect to it and navigate to the captive portal to enter:

- WiFi SSID and password
- MQTT host, port, username, and password
- MQTT device name and ID

Configuration is stored in NVS and loaded on subsequent boots.

### Building and Flashing

Requires the Rust ESP-IDF toolchain. Set it up with [esp-idf](https://github.com/esp-rs/esp-idf) prerequisites.

```sh
cargo build --release -p plant-tower
cargo espflash flash --release -p plant-tower
```
