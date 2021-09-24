use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Document {
    /// Required fields
    pub id: Uuid,
    // If updating an existing doc, this will point to the `id` of the original document, and
    // the revision field should be incremented
    pub origid: Uuid,
    pub authors: Vec<String>,
    // Note the custom Serialize implementation below to skip the `body` if the
    // skip_serializing_body attribute is set
    pub body: String,
    #[serde(default)]
    #[serde(skip)]
    pub skip_serializing_body: bool,
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
    pub slug: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub tag: Vec<String>,
    #[serde(default)]
    pub weight: i32,
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let toml = toml::to_string(&self).unwrap();
        write!(f, "+++\n{}+++\n{}", toml, self.body)
    }
}

impl From<markdown_fm_doc::Document> for Document {
    fn from(item: markdown_fm_doc::Document) -> Self {
        let uuid = Uuid::new_v4();
        Document {
            id: uuid,
            origid: uuid,
            authors: vec![item.author],
            body: item.body,
            date: item.date,
            latest: true,
            revision: 1,
            tag: item.tags,
            title: item.title,
            subtitle: item.subtitle,
            ..Default::default()
        }
    }
}

// Custom Serialization to skip body attribute if requested
impl Serialize for Document {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        //let mut s = serializer.serialize_struct("Document", 12)?;
        let mut s = if self.skip_serializing_body {
            serializer.serialize_struct("Document", 12)?
        } else {
            let mut s = serializer.serialize_struct("Document", 13)?;
            s.serialize_field("body", &self.body)?;
            s
        };
        s.serialize_field("id", &self.id)?;
        s.serialize_field("origid", &self.origid)?;
        s.serialize_field("authors", &self.authors)?;
        s.serialize_field("date", &self.date)?;
        s.serialize_field("latest", &self.latest)?;
        s.serialize_field("revision", &self.revision)?;
        s.serialize_field("title", &self.title)?;
        s.serialize_field("background_img", &self.background_img)?;
        s.serialize_field("links", &self.links)?;
        s.serialize_field("slug", &self.slug)?;
        s.serialize_field("subtitle", &self.subtitle)?;
        s.serialize_field("tag", &self.tag)?;
        s.serialize_field("weight", &self.weight)?;
        s.end()
    }
}
