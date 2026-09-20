use embedded_hal::blocking::delay::DelayMs;
use esp_idf_hal::delay::Ets;
use esp_idf_hal::gpio::Pull;
use esp_idf_hal::gpio::{InputOutput, InputPin, OutputPin, PinDriver};
use onewire::OneWire;

pub struct OneWireTemperatureSensor<'a> {
    pin: PinDriver<'a, InputOutput>,
}

impl<'a> OneWireTemperatureSensor<'a> {
    pub fn new<P: InputPin + OutputPin + 'a>(pin: P) -> Self {
        let pin_number = pin.pin();
        let mut pin_driver = PinDriver::input_output_od(pin, Pull::Floating)
            .expect("Failed to initialize OneWire pin");
        let present = critical_section::with(|_| {
            let mut delay = Ets;
            let mut wire = OneWire::new(&mut pin_driver, false);
            wire.reset(&mut delay).unwrap_or(false)
        });
        assert!(
            present,
            "No OneWire device on pin {} - check wiring and pull-up resistor",
            pin_number
        );
        Self { pin: pin_driver }
    }

    pub fn get_temperature(&mut self) -> Option<f32> {
        self.start_conversion()?;
        Ets.delay_ms(750u32);
        self.read_scratchpad()
    }

    fn start_conversion(&mut self) -> Option<()> {
        critical_section::with(|_| {
            let mut delay = Ets;
            let mut wire = OneWire::new(&mut self.pin, false);
            wire.reset(&mut delay).ok()?;
            wire.write_bytes(&mut delay, &[0xCC, 0x44]).ok()?;
            Some(())
        })
    }

    fn read_scratchpad(&mut self) -> Option<f32> {
        critical_section::with(|_| {
            let mut delay = Ets;
            let mut wire = OneWire::new(&mut self.pin, false);
            wire.reset(&mut delay).ok()?;
            wire.write_bytes(&mut delay, &[0xCC, 0xBE]).ok()?;
            let mut data = [0u8; 9];
            wire.read_bytes(&mut delay, &mut data).ok()?;
            let raw = (data[1] as u16) << 8 | data[0] as u16;
            Some(raw as i16 as f32 / 16.0)
        })
    }
}
