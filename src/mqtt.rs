use crate::configuration::MqttConfig;
use rumqttc::{AsyncClient, ConnAck, Event, Incoming, MqttOptions, Publish, QoS, SubscribeFilter};
use std::{sync::Arc, time::Duration};
use tokio::sync::{
    mpsc::{channel, Receiver},
    Notify,
};
use tracing::info;

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

pub async fn start_mqtt_service_with_subs(
    config: &MqttConfig,
    subscribers: Vec<String>,
) -> anyhow::Result<(AsyncClient, Receiver<Publish>)> {
    // weird method
    let mut mqttoptions =
        MqttOptions::new(&config.client_id, &config.broker_host, config.broker_port);
    info!("Starting MQTT server with options {:?}", mqttoptions);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    let (sender, receiver) = channel(10);
    let client_clone = client.clone();

    // make sure we got at least one message
    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();

    tokio::spawn(async move {
        let client = client_clone;

        let subscriber_filters = subscribers.into_iter().map(|topic| SubscribeFilter {
            path: topic,
            qos: QoS::AtMostOnce,
        });

        client
            .subscribe_many(subscriber_filters.clone())
            .await
            .unwrap();

        loop {
            match eventloop.poll().await {
                Ok(notification) => match notification {
                    Event::Incoming(Incoming::Publish(publish)) => {
                        if let Err(e) = sender.send(publish).await {
                            eprintln!("Error sending message {e}");
                        }
                        notify_clone.notify_waiters();
                    }
                    Event::Incoming(Incoming::ConnAck(_)) => {
                        client
                            .subscribe_many(subscriber_filters.clone())
                            .await
                            .unwrap();
                    }
                    _ => (),
                },
                Err(e) => {
                    eprintln!("Error processing eventloop notifications {e}");
                }
            }
        }
    });

    // meh
    notify.notified().await;

    Ok((client, receiver))
}
