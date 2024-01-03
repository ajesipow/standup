use std::fs;
use std::thread::sleep;
use std::time::Duration;

use anyhow::anyhow;
use anyhow::Result;
use log::debug;
use log::info;

use crate::config::Config;
use crate::config::TableConfig;
use crate::motor::DeskMotor;
use crate::motor::Motor;
use crate::movement::Movement;
use crate::primitives::Centimeter;
use crate::sensor::DistanceSensor;
use crate::sensor::HCSR04;

/// The standing desk implementation.
#[derive(Debug)]
pub(crate) struct StandingDesk<S: DistanceSensor = HCSR04, M: Motor = DeskMotor> {
    config: TableConfig,
    sensor: S,
    motor: M,
}

impl StandingDesk {
    /// Creates a new instance of a standing desk.
    pub fn new(config: Config) -> Self {
        let sensor = HCSR04::new(config.sensor);
        let motor = DeskMotor::new(config.motor);
        Self {
            config: config.table,
            sensor,
            motor,
        }
    }
}

impl<S: DistanceSensor, M: Motor> Movement for StandingDesk<S, M> {
    fn move_to_standing(&mut self) -> Result<()> {
        info!("Moving to standing position ...");
        self.move_to_height(self.config.standing_height)
    }

    fn move_to_sitting(&mut self) -> Result<()> {
        info!("Moving to standing position ...");
        self.move_to_height(self.config.sitting_height)
    }

    fn calibrate(&mut self) -> Result<()> {
        info!("Calibrating");
        self.motor.up();
        let mut current_height = self.sensor.current_height()?;
        // We subtract a bit to kick-start the while loop below
        let mut previous_height = current_height - Centimeter(1);
        // TODO add timeout
        while previous_height < current_height {
            // Table is still moving
            sleep(Duration::from_millis(200));
            previous_height = current_height;
            current_height = self.sensor.current_height()?;
        }
        self.motor.stop();
        self.sensor.set_max_height(self.config.max_table_height)?;

        self.motor.down();
        // TODO add timeout
        // We add a bit to kick-start the while loop below
        previous_height = current_height + Centimeter(1);
        while previous_height > current_height {
            // Table is still moving down
            sleep(Duration::from_millis(200));
            previous_height = current_height;
            current_height = self.sensor.current_height()?;
        }
        self.motor.stop();
        self.sensor.set_min_height(self.config.min_table_height)?;

        let calibration_file = self.sensor.calibration_file();
        let raw_calibration_data = toml::to_string(&self.sensor.calibration_data())?;
        fs::write(calibration_file, raw_calibration_data)?;
        debug!("Calibration data written to {calibration_file:?}");

        self.move_to_sitting()
    }

    fn move_to_height(
        &mut self,
        height_cm: Centimeter,
    ) -> Result<()> {
        if height_cm > self.config.max_table_height {
            return Err(anyhow!(
                "Cannot move table higher than {:?}",
                self.config.max_table_height
            ));
        } else if height_cm < self.config.min_table_height {
            return Err(anyhow!(
                "Cannot move table lower than {:?}",
                self.config.min_table_height
            ));
        }
        info!("Moving to height {height_cm:?}");
        let current_height = self.sensor.current_height()?;
        // We allow for some tolerance as moving the table is not so precise
        if height_cm - Centimeter(1) <= current_height
            && current_height <= height_cm + Centimeter(1)
        {
            debug!("Table already at desired height");
            return Ok(());
        }
        // TODO add timeout
        if current_height < height_cm {
            self.motor.up();
            while self.sensor.current_height()? < height_cm {
                sleep(Duration::from_millis(200));
            }
            self.motor.stop();
        }
        // TODO add timeout
        if current_height > height_cm {
            self.motor.down();
            while self.sensor.current_height()? > height_cm {
                sleep(Duration::from_millis(200));
            }
            self.motor.stop();
        }
        Ok(())
    }
}
