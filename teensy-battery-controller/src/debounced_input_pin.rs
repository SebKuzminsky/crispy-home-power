pub struct DebouncedInputPin<PIN>
where
    PIN: embedded_hal::digital::InputPin,
{
    input_pin: PIN,
    current_state: bool,
    count: u16,
    count_needed_to_change: u16,
}

impl<PIN> DebouncedInputPin<PIN>
where
    PIN: embedded_hal::digital::InputPin,
{
    pub fn new(mut input_pin: PIN, count_needed_to_change: u16) -> Self {
        let current_state = input_pin.is_high().unwrap();
        Self {
            input_pin,
            current_state,
            count: 0,
            count_needed_to_change,
        }
    }
}

impl<PIN> embedded_hal::digital::ErrorType for DebouncedInputPin<PIN>
where
    PIN: embedded_hal::digital::InputPin,
{
    type Error = core::convert::Infallible;
}

impl<PIN> embedded_hal::digital::InputPin for DebouncedInputPin<PIN>
where
    PIN: embedded_hal::digital::InputPin,
{
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        let new_state = self.input_pin.is_high().unwrap();
        if new_state == self.current_state {
            self.count = 0;
        } else {
            self.count += 1;
        }
        if self.count > self.count_needed_to_change {
            self.count = 0;
            self.current_state = new_state;
        }
        Ok(self.current_state)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        let Ok(current_state) = self.is_high();
        Ok(!current_state)
    }
}
