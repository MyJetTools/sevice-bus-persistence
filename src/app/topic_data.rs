use std::{collections::HashMap, sync::Arc};

use my_service_bus_shared::page_id::PageId;
use tokio::sync::{Mutex, RwLock};

use crate::{
    app::AppContext,
    message_pages::{MessagePageId, MessagesPage, PageWriterMetrics},
};

use super::current_pages_cluster::CurrentPagesCluster;

pub struct TopicData {
    pub topic_id: String,
    pub pages: Mutex<HashMap<PageId, Arc<MessagesPage>>>,
    pub metrics: RwLock<PageWriterMetrics>,
    pub app: Arc<AppContext>,
    pub pages_cluster: CurrentPagesCluster,
}

impl TopicData {
    pub fn new(topic_id: &str, app: Arc<AppContext>) -> Self {
        Self {
            topic_id: topic_id.to_string(),
            pages: Mutex::new(HashMap::new()),
            metrics: RwLock::new(PageWriterMetrics::new()),
            app: app.clone(),
            pages_cluster: CurrentPagesCluster::new(app, topic_id.to_string()),
        }
    }

    pub async fn get(&self, page_id: MessagePageId) -> Option<Arc<MessagesPage>> {
        let pages_access = self.pages.lock().await;
        let result = pages_access.get(&page_id.value)?;

        Some(result.clone())
    }

    pub async fn remove_page(&self, page_id: i64) -> Option<Arc<MessagesPage>> {
        let mut pages_access = self.pages.lock().await;
        pages_access.remove(&page_id)
    }

    pub async fn try_get_or_create_uninitialized(
        &self,
        page_id: MessagePageId,
    ) -> Arc<MessagesPage> {
        let mut pages_access = self.pages.lock().await;

        if pages_access.contains_key(&page_id.value) {
            return pages_access.get(&page_id.value).unwrap().clone();
        }

        pages_access.insert(page_id.value, Arc::new(MessagesPage::brand_new(page_id)));

        return pages_access.get(&page_id.value).unwrap().clone();
    }

    pub async fn has_messages_to_save(&self) -> bool {
        let pages_access = self.pages.lock().await;

        for page in pages_access.values() {
            if page.as_ref().has_messages_to_save().await {
                return true;
            }
        }

        false
    }

    pub async fn get_pages_with_data_to_save(&self) -> Vec<Arc<MessagesPage>> {
        let mut result = Vec::new();

        let pages_access = self.pages.lock().await;

        for page in pages_access.values() {
            let has_messages_to_save = page.has_messages_to_save().await;

            if has_messages_to_save {
                result.push(page.clone());
            }
        }

        return result;
    }

    pub async fn get_metrics(&self) -> PageWriterMetrics {
        let metrics_access = self.metrics.read().await;
        return metrics_access.clone();
    }

    pub async fn get_queue_size(&self) -> usize {
        let pages_access = self.pages.lock().await;

        let mut result = 0;
        for page in pages_access.values() {
            result += page.get_messages_to_save_amount().await;
        }

        result
    }

    /*
       pub async fn add_new_messages(
           &self,
           page_id: MessagePageId,
           messages: &[MessageProtobufModel],
       ) {
       }
    */

    pub async fn get_all(&self) -> Vec<Arc<MessagesPage>> {
        let mut result = Vec::new();
        let read_access = self.pages.lock().await;

        for page in read_access.values() {
            let itm = page.clone();
            result.push(itm);
        }

        result
    }
}
