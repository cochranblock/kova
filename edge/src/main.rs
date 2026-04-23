// Unlicense — cochranblock.org
// Contributors: GotEmCoach, KOVA, Claude Opus 4.6
//! kova-edge: ESP32-WROOM-32 firmware.
//! Reads BME280 (temp/humidity/pressure) over I2C, prints CSV over serial.
//! Phase 1: sensor → serial. Phase 2: on-device inference.

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::prelude::*;

use bme280::i2c::BME280;

fn main() {
    esp_idf_sys::link_patches();

    let peripherals = Peripherals::take().unwrap();

    let i2c = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
        &I2cConfig::new().baudrate(100.kHz().into()),
    )
    .unwrap();

    let mut bme = BME280::new_primary(i2c);
    bme.init(&mut FreeRtos).unwrap();

    loop {
        match bme.measure(&mut FreeRtos) {
            Ok(m) => {
                println!("{:.1},{:.1},{:.0}", m.temperature, m.humidity, m.pressure);
            }
            Err(e) => {
                println!("E:{:?}", e);
            }
        }
        FreeRtos::delay_ms(2000);
    }
}
