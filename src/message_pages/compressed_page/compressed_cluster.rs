use std::{collections::BTreeMap, sync::Arc};

use my_service_bus_shared::{
    page_compressor::{CompressedPageBuilder, CompressedPageReader, CompressedPageReaderError},
    protobuf_models::MessageProtobufModel,
    MessageId,
};
use tokio::sync::Mutex;

use crate::{
    app::Logs,
    page_blob_random_access::{PageBlobPageId, PageBlobRandomAccess},
    toc::{ContentOffset, FileToc},
};

use super::{utils::*, CompressedClusterId, CompressedPageId};

pub struct CompressedClusterData {
    toc: FileToc,
    page_blob: PageBlobRandomAccess,
}

impl CompressedClusterData {
    pub fn new(toc: FileToc, page_blob: PageBlobRandomAccess) -> Self {
        Self { toc, page_blob }
    }
}

pub struct CompressedCluster {
    pub page_cluster_id: CompressedClusterId,
    data: Mutex<CompressedClusterData>,
    logs: Arc<Logs>,
    max_message_size: usize,
    topic_id: String,
}

impl CompressedCluster {
    pub async fn new(
        topic_id: String,
        page_cluster_id: CompressedClusterId,
        mut page_blob: PageBlobRandomAccess,
        max_message_size: usize,
        logs: Arc<Logs>,
    ) -> Self {
        let toc = FileToc::read_toc(
            &mut page_blob,
            COMPRESSED_CLUSTER_TOC_IN_PAGES,
            COMPRESSED_CLUSTER_TOC,
            PAGES_PER_CLUSTER,
        )
        .await;

        Self {
            topic_id,
            page_cluster_id,
            data: Mutex::new(CompressedClusterData::new(toc, page_blob)),
            max_message_size,
            logs,
        }
    }

    pub async fn has_compressed_page(&self, compressed_page_id: &CompressedPageId) -> bool {
        let data = self.data.lock().await;
        let page_id_within_cluster = compressed_page_id.get_page_id_within_cluster();
        data.toc.has_content(page_id_within_cluster)
    }

    async fn get_compressed_page_payload(
        &self,
        compressed_page_id: &CompressedPageId,
    ) -> Option<Vec<u8>> {
        let mut data = self.data.lock().await;
        let toc = data.toc.get_position(compressed_page_id.value);

        if toc.has_data(self.max_message_size) {
            return None;
        }

        let content = data
            .page_blob
            .read_from_position(toc.offset, toc.size)
            .await;

        Some(content.as_slice().to_vec())
    }

    pub async fn get_compressed_page_messages(
        &self,
        compressed_page_id: &CompressedPageId,
    ) -> Result<Option<BTreeMap<MessageId, MessageProtobufModel>>, CompressedPageReaderError> {
        let compressed_payload = self.get_compressed_page_payload(compressed_page_id).await;

        if compressed_payload.is_none() {
            return Ok(None);
        }

        let compressed_payload = compressed_payload.unwrap();

        let mut compressed_page_reader = CompressedPageReader::new(compressed_payload)?;

        let mut result = BTreeMap::new();

        while let Some(next_message) = compressed_page_reader.get_next_message()? {
            match prost::Message::decode(next_message.1.as_slice()) {
                Ok(message) => {
                    result.insert(next_message.0, message);
                }
                Err(err) => self.logs.add_error_str(
                    Some(self.topic_id.as_str()),
                    "get_compressed_page",
                    format!("Can not decode message {}", next_message.0),
                    format!("{:?}", err),
                ),
            }
        }

        Ok(Some(result))
    }

    pub async fn save_cluser_page(&self, messages: &[Arc<MessageProtobufModel>]) {
        let compressed_page_id =
            CompressedPageId::from_message_id(messages.first().unwrap().message_id);

        let mut data = self.data.lock().await;

        if data.toc.has_content(compressed_page_id.value) {
            return;
        }

        let mut compressed_page_builder = CompressedPageBuilder::new();

        for message in messages {
            let mut payload = Vec::new();
            prost::Message::encode(message.as_ref(), &mut payload).unwrap();
            compressed_page_builder
                .add_message(message.message_id, payload.as_slice())
                .unwrap();
        }

        let compressed_content = compressed_page_builder.get_payload().unwrap();

        let offset = ContentOffset {
            offset: data.toc.get_write_position(),
            size: compressed_content.len(),
        };

        data.page_blob
            .write_at_position(offset.offset, compressed_content.as_slice(), 1)
            .await;

        if let Some(page_from) = data
            .toc
            .update_file_position(compressed_page_id.value, &offset)
        {
            let toc_content = data.toc.get_toc_pages(page_from, 1).to_vec();

            data.page_blob
                .save_pages(&PageBlobPageId::new(page_from), toc_content.as_slice())
                .await;
        }

        data.toc.increase_write_position(offset.size);
    }
}
