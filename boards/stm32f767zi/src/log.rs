use crate::tasks::mqtt::send::MQTT_SEND;
use defmt::{debug, error, info, warn};
use embassy_time::{Duration, Timer};
use hyped_core::{log_types::LogLevel, mqtt::MqttMessage, mqtt_topics::MqttTopic};

/// Log a message to the console and send it to the MQTT broker
pub async fn log(level: LogLevel, message: &str) {
    match level {
        LogLevel::Info => info!("{}", message),
        LogLevel::Warn => warn!("{}", message),
        LogLevel::Error => error!("{}", message),
        LogLevel::Debug => debug!("{}", message),
    }
    MQTT_SEND
        .send(MqttMessage::new_json_string(MqttTopic::Logs, message))
        .await;
}

#[derive(Clone, Copy)]
pub enum LogSender {
    Can,
    Uart,
    SdCard,
    BaseStation,
}

const CHUNK_SIZE: usize = 64;
const IDLE_DELAY: Duration = Duration::from_millis(5);

#[embassy_executor::task]
pub async fn log_send(sinks: &'static [LogSender]) {
    let mut chunk = [0u8; CHUNK_SIZE];
    loop {
        let n = hyped_logger::read(&mut chunk);
        if n == 0 {
            Timer::after(IDLE_DELAY).await;
            continue;
        }

        let bytes = &chunk[..n];

        for sink in sinks {
            match sink {
                LogSender::Can => send_to_can(bytes).await,
                LogSender::Uart => send_to_uart(bytes).await,
                LogSender::SdCard => send_to_sd_card(bytes).await,
                LogSender::BaseStation => send_to_base_station(bytes).await,
            }
        }
    }
}

// Will be implemented later
async fn send_to_can(_bytes: &[u8]) {}
async fn send_to_uart(_bytes: &[u8]) {}
async fn send_to_sd_card(_bytes: &[u8]) {}
async fn send_to_base_station(_bytes: &[u8]) {}
