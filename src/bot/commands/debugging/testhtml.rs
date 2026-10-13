use super::send;
use crate::bot::commands::{Command, Context, Scope};
use anyhow::Result;
use serde_json::json;
use whatsapp_rust::prelude::{MessageField, wa};

pub(in crate::bot::commands) const COMMAND: Command = Command {
    name: "testhtml",
    aliases: &["sendhtml"],
    category: "debugging",
    description: "commands.testhtml",
    cooldown_secs: 3,
    limit_cost: 0,
    scope: Scope::Both,
    owner_only: true,
    handler: |context| Box::pin(execute(context)),
};

const TITLE: &str = "WhatsApp BOT by TobyG74";
const HTML: &str = r#"<div style="box-sizing:border-box;width:100%;padding:24px;background:#f5faf7;border-left:4px solid #16834a;border-radius:12px;font-family:Arial,sans-serif;color:#162b22;overflow-wrap:anywhere;">
  <h1 style="margin:0 0 8px;font-size:28px;line-height:1.2;">WhatsApp BOT</h1>
  <p style="margin:0;font-size:16px;line-height:1.5;">by <strong>TobyG74</strong></p>
  <hr style="margin:20px 0;border:0;border-top:1px solid #ccdcd2;">
  <p style="margin:0;font-size:14px;line-height:1.6;">A test message with <strong>bold text</strong>, <em>italic text</em>, and a custom HTML layout.</p>
</div>"#;

fn build() -> Result<wa::Message> {
    let response_id = format!("rich-html-{}", uuid::Uuid::new_v4());
    let payload = json!({
        "response_id": response_id,
        "sections": [{"view_model": {
            "primitive": {"__typename": "GenAIaeacdsnwHtmlPrimitive", "payload": HTML, "trusted_sources": ["testsource"]},
            "__typename": "GenAISingleLayoutViewModel",
        }}],
    });
    Ok(wa::Message {
        message_context_info: MessageField::some(wa::MessageContextInfo {
            device_list_metadata: MessageField::some(wa::DeviceListMetadata::default()),
            device_list_metadata_version: Some(2),
            bot_metadata: MessageField::some(wa::BotMetadata {
                message_disclaimer_text: Some(String::new()),
                bot_response_id: Some(response_id),
                ..Default::default()
            }),
            ..Default::default()
        }),
        bot_forwarded_message: MessageField::some(wa::message::FutureProofMessage {
            message: MessageField::some(wa::Message {
                rich_response_message: MessageField::some(wa::AIRichResponseMessage {
                    message_type: Some(
                        wa::AIRichResponseMessageType::AI_RICH_RESPONSE_TYPE_STANDARD,
                    ),
                    submessages: vec![wa::AIRichResponseSubMessage {
                        message_type: Some(wa::AIRichResponseSubMessageType::AI_RICH_RESPONSE_TEXT),
                        message_text: Some(TITLE.into()),
                        ..Default::default()
                    }],
                    unified_response: MessageField::some(wa::AIRichResponseUnifiedResponse {
                        data: Some(serde_json::to_vec(&payload)?),
                    }),
                    context_info: MessageField::some(wa::ContextInfo {
                        forwarding_score: Some(1),
                        is_forwarded: Some(true),
                        forwarded_ai_bot_message_info: MessageField::some(
                            wa::ForwardedAIBotMessageInfo {
                                // The reference rich-response envelope uses this protocol bot identity.
                                bot_jid: Some("867051314767696@bot".into()),
                                ..Default::default()
                            },
                        ),
                        forward_origin: Some(wa::context_info::ForwardOrigin::META_AI),
                        ..Default::default()
                    }),
                }),
                ..Default::default()
            }),
        }),
        ..Default::default()
    })
}

async fn execute(context: Context<'_>) -> Result<()> {
    let message = build()?;
    send(context, message).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::wacore::messages::{MessageUtils, decode_plaintext};

    #[test]
    fn html_uses_text_stanzas_for_groups_and_private_chats() {
        use whatsapp_rust::{StanzaType, wacore::send::stanza_type_from_message};

        let message = build().unwrap();
        // Reproduce the default classification that differs from Baileys.
        assert_eq!(stanza_type_from_message(&message), "media");
        for group in [false, true] {
            let options = super::super::send_options(&message, group);
            assert_eq!(options.stanza_type_override, Some(StanzaType::Text));
            assert!(options.extra_stanza_nodes.is_empty());
        }
    }

    #[test]
    fn html_survives_group_and_private_send_encoding() {
        let message = build().unwrap();
        let context = wa::MessageContextInfo {
            message_secret: Some(vec![42; 32]),
            reporting_token_version: Some(2),
            ..Default::default()
        };
        let group = decode_plaintext(
            &MessageUtils::encode_and_pad_with_context(&message, Some(&context)),
            2,
        )
        .unwrap();
        let private = MessageUtils::encode_dm_plaintexts(
            &message,
            Some(&context),
            "123456789@s.whatsapp.net",
        );
        let recipient = decode_plaintext(&private.recipient, 2).unwrap();
        let own = decode_plaintext(&private.own_devices, 2).unwrap();
        for decoded in [&group, &recipient, &own] {
            let metadata = decoded.message_context_info.as_option().unwrap();
            assert_eq!(metadata.message_secret, context.message_secret);
            assert_eq!(
                metadata.reporting_token_version,
                context.reporting_token_version
            );
            assert_eq!(
                metadata.bot_metadata,
                message
                    .message_context_info
                    .as_option()
                    .unwrap()
                    .bot_metadata
            );
            let content = decoded
                .device_sent_message
                .as_option()
                .map(|dsm| dsm.message.as_option().unwrap())
                .unwrap_or(decoded);
            assert_eq!(content.bot_forwarded_message, message.bot_forwarded_message);
        }
    }

    #[test]
    fn html_is_encoded_inside_the_rich_response_with_matching_ids() {
        let message = build().unwrap();
        let id = message
            .message_context_info
            .as_option()
            .unwrap()
            .bot_metadata
            .as_option()
            .unwrap()
            .bot_response_id
            .as_deref()
            .unwrap();
        let rich = message
            .bot_forwarded_message
            .as_option()
            .unwrap()
            .message
            .as_option()
            .unwrap()
            .rich_response_message
            .as_option()
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(
            rich.unified_response
                .as_option()
                .unwrap()
                .data
                .as_ref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(payload["response_id"], id);
        assert_eq!(
            payload["sections"][0]["view_model"]["primitive"]["payload"],
            HTML
        );
        assert_eq!(
            rich.context_info.as_option().unwrap().forward_origin,
            Some(wa::context_info::ForwardOrigin::META_AI)
        );
        assert_eq!(rich.submessages[0].message_text.as_deref(), Some(TITLE));
        assert!(HTML.contains("WhatsApp BOT"));
        assert!(HTML.contains("TobyG74"));
        assert!(HTML.len() <= 16 * 1024);
        assert_ne!(
            build()
                .unwrap()
                .message_context_info
                .as_option()
                .unwrap()
                .bot_metadata
                .as_option()
                .unwrap()
                .bot_response_id
                .as_deref(),
            Some(id)
        );
    }
}
