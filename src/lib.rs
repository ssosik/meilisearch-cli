use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Required fields
    pub id: Uuid,
    // If updating an existing doc, this will point to the `id` of the original document, and
    // the revision field should be incremented
    pub origid: Uuid,
    pub authors: Vec<String>,
    pub body: String,
    pub date: String,
    pub latest: bool,
    pub revision: u16,
    pub title: String,
    /// Optional fields
    #[serde(default)]
    pub background_img: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub weight: i32,
}

impl From<markdown_fm_doc::Document> for Document {
    fn from(item: markdown_fm_doc::Document) -> Self {
        //let uuid =Uuid::new_v4().to_hyphenated().to_string();
        let uuid = Uuid::new_v4();
        Document {
            id: uuid,
            origid: uuid,
            authors: vec![item.author],
            body: item.body,
            date: item.date,
            latest: true,
            revision: 1,
            tags: item.tags,
            title: item.title,
            subtitle: item.subtitle,
            ..Default::default()
        }
    }
}
