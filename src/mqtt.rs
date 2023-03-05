use crate::configuration::MqttConfig;
use rumqttc::{AsyncClient, ConnAck, MqttOptions, Publish};
use std::time::Duration;
use tracing::*;

enum MqttUpdate {
    Message(Publish),
    Reconnection(ConnAck),
}

pub fn start_mqtt_service(config: &MqttConfig) -> anyhow::Result<AsyncClient> {
    let mut mqttoptions =
        MqttOptions::new(&config.client_id, &config.broker_host, config.broker_port);
    info!("Starting MQTT server with options {:?}", mqttoptions);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(_notification) => (),
                Err(e) => {
                    eprintln!("Error processing eventloop notifications {e}");
                }
            }
        }
    });

    Ok(client)
}
