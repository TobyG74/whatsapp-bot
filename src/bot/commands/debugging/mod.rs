use super::Context;
use anyhow::Result;
use serde_json::Value;
use std::sync::atomic::Ordering;
use wa::message::interactive_message::{self, native_flow_message::NativeFlowButton};
use whatsapp_rust::{
    StanzaType,
    prelude::{MessageField, wa},
    send::SendOptions,
    wacore_binary::{builder::NodeBuilder, node::Node},
};

pub(super) mod testbutton;
pub(super) mod testhtml;
pub(super) mod testinteractive;
pub(super) mod testlist;

fn button(name: &str, params: Value) -> NativeFlowButton {
    NativeFlowButton {
        name: Some(name.into()),
        button_params_json: Some(params.to_string()),
    }
}

fn native(title: &str, body: &str, footer: &str, buttons: Vec<NativeFlowButton>) -> wa::Message {
    wa::Message {
        view_once_message: MessageField::some(wa::message::FutureProofMessage {
            message: MessageField::some(wa::Message {
                message_context_info: MessageField::some(wa::MessageContextInfo {
                    device_list_metadata: MessageField::some(wa::DeviceListMetadata::default()),
                    device_list_metadata_version: Some(2),
                    ..Default::default()
                }),
                interactive_message: MessageField::some(wa::message::InteractiveMessage {
                    header: MessageField::some(interactive_message::Header {
                        title: Some(title.into()),
                        has_media_attachment: Some(false),
                        ..Default::default()
                    }),
                    body: MessageField::some(interactive_message::Body {
                        text: Some(body.into()),
                    }),
                    footer: MessageField::some(interactive_message::Footer {
                        text: Some(footer.into()),
                        ..Default::default()
                    }),
                    interactive_message: Some(
                        interactive_message::InteractiveMessage::NativeFlowMessage(Box::new(
                            interactive_message::NativeFlowMessage {
                                buttons,
                                message_params_json: Some(String::new()),
                                message_version: Some(1),
                            },
                        )),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        }),
        ..Default::default()
    }
}

fn native_nodes(group: bool) -> Vec<Node> {
    // whatsapp-rust 0.7 only infers these nodes for unwrapped interactive payloads.
    let privacy_ts = chrono::Utc::now()
        .timestamp()
        .saturating_sub(77_980_457)
        .to_string();
    let mut nodes = Vec::new();
    if !group {
        nodes.push(NodeBuilder::new("bot").attr("biz_bot", "1").build());
    }
    nodes.push(
        NodeBuilder::new("biz")
            .attr("actual_actors", "2")
            .attr("host_storage", "2")
            .attr("privacy_mode_ts", privacy_ts)
            .children([
                NodeBuilder::new("interactive")
                    .attr("type", "native_flow")
                    .attr("v", "1")
                    .children([NodeBuilder::new("native_flow")
                        .attr("name", "mixed")
                        .attr("v", "9")
                        .build()])
                    .build(),
                NodeBuilder::new("quality_control")
                    .attr("source_type", "third_party")
                    .build(),
            ])
            .build(),
    );
    nodes
}

fn send_options(message: &wa::Message, group: bool) -> SendOptions {
    if message.bot_forwarded_message.as_option().is_some() {
        // whatsapp-rust 0.7 classifies this envelope as media; Chisato relays it as text.
        SendOptions::default().with_stanza_type_override(StanzaType::Text)
    } else if message.view_once_message.as_option().is_some() {
        SendOptions::default().with_extra_stanza_nodes(native_nodes(group))
    } else {
        SendOptions::default()
    }
}

async fn send(context: Context<'_>, message: wa::Message) -> Result<()> {
    let options = send_options(&message, context.message.info.source.is_group);
    context
        .message
        .client
        .send_message_with_options(&context.message.info.source.chat, message, options)
        .await?;
    context.state.sent.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::config::{Config, Mode};
    use crate::bot::commands::{allowed, parse};

    #[test]
    fn native_payloads_and_routing_nodes_match_the_protocol() {
        let settings = toml::from_str::<Config>(include_str!("../../../../config.example.toml"))
            .unwrap()
            .bot;
        for (name, message, kinds) in [
            (
                "testbutton",
                testbutton::build(&settings),
                vec!["quick_reply", "quick_reply"],
            ),
            (
                "testlist",
                testlist::build(&settings),
                vec!["single_select"],
            ),
            (
                "testinteractive",
                testinteractive::build(&settings),
                vec!["quick_reply", "cta_url", "cta_copy"],
            ),
        ] {
            let wrapper = message.view_once_message.as_option().unwrap();
            for group in [false, true] {
                let options = send_options(&message, group);
                assert!(options.stanza_type_override.is_none());
                assert_eq!(options.extra_stanza_nodes.len(), if group { 1 } else { 2 });
            }
            let inner = wrapper.message.as_option().unwrap();
            assert_eq!(
                inner
                    .message_context_info
                    .as_option()
                    .unwrap()
                    .device_list_metadata_version,
                Some(2)
            );
            let im = inner.interactive_message.as_option().unwrap();
            assert!(
                !im.body
                    .as_option()
                    .unwrap()
                    .text
                    .as_ref()
                    .unwrap()
                    .is_empty()
            );
            let Some(interactive_message::InteractiveMessage::NativeFlowMessage(flow)) =
                &im.interactive_message
            else {
                panic!("missing native flow")
            };
            assert_eq!(
                flow.buttons
                    .iter()
                    .map(|b| b.name.as_deref().unwrap())
                    .collect::<Vec<_>>(),
                kinds
            );
            for b in &flow.buttons {
                let params: Value =
                    serde_json::from_str(b.button_params_json.as_ref().unwrap()).unwrap();
                if let Some(id) = params["id"].as_str() {
                    assert!(parse(id, &settings).is_some());
                }
                if b.name.as_deref() == Some("single_select") {
                    for row in params["sections"][0]["rows"].as_array().unwrap() {
                        assert!(parse(row["id"].as_str().unwrap(), &settings).is_some());
                    }
                }
            }
            let (command, options, _) = parse(&format!("!{name}"), &settings).unwrap();
            assert_eq!(command.category, "debugging");
            assert!(!allowed(&options, false, true, Mode::Public));
            assert!(allowed(&options, true, true, Mode::Public));
            assert!(allowed(&options, true, false, Mode::SelfOnly));
        }
        let private = native_nodes(false);
        assert_eq!(
            private
                .iter()
                .map(|n| n.tag.as_ref())
                .collect::<Vec<&str>>(),
            ["bot", "biz"]
        );
        let group = native_nodes(true);
        assert_eq!(group.len(), 1);
        let interactive = group[0].get_optional_child("interactive").unwrap();
        assert_eq!(
            interactive
                .get_optional_child("native_flow")
                .unwrap()
                .attrs()
                .optional_string("name")
                .unwrap()
                .as_ref(),
            "mixed"
        );
    }
}
