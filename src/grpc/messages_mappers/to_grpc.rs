use my_service_bus_shared::protobuf_models::{MessageMetaDataProtobufModel, MessageProtobufModel};

use crate::persistence_grpc::*;

impl<'s> Into<MessageContentGrpcModel> for &'s MessageProtobufModel {
    fn into(self) -> MessageContentGrpcModel {
        MessageContentGrpcModel {
            data: self.data.clone(),
            created: self.get_created().unix_microseconds,
            message_id: self.get_message_id().into(),
            meta_data: self.headers.iter().map(|itm| itm.into()).collect(),
        }
    }
}

impl<'s> Into<MessageContentMetaDataItem> for &'s MessageMetaDataProtobufModel {
    fn into(self) -> MessageContentMetaDataItem {
        MessageContentMetaDataItem {
            key: self.key.to_string(),
            value: self.value.to_string(),
        }
    }
}

impl Into<MessageContentMetaDataItem> for MessageMetaDataProtobufModel {
    fn into(self) -> MessageContentMetaDataItem {
        MessageContentMetaDataItem {
            key: self.key,
            value: self.value,
        }
    }
}
