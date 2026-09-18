use num_traits::NumCast;

pub struct MockSensor<T> {
    min: T,
    max: T,
    _marker: std::marker::PhantomData<T>,
}

impl<T: NumCast + num_traits::ToPrimitive> MockSensor<T> {
    pub fn new(min: T, max: T) -> Self {
        Self {
            min,
            max,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn get_value(&self) -> T {
        let raw = unsafe { esp_idf_svc::sys::esp_random() };
        let min = self.min.to_u32().unwrap();
        let max = self.max.to_u32().unwrap();
        T::from(min + raw % (max - min + 1)).expect("Failed to cast random value")
    }
}
