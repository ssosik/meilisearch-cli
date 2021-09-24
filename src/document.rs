use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use std::fmt;
use unicode_width::UnicodeWidthStr;
use uuid_b64::UuidB64;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Document {
    /// Required fields
    pub id: String,
    // If updating an existing doc, this will point to the `id` of the original document, and
    // the revision field should be incremented
    pub origid: String,
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
    #[serde(default)]
    pub filename: String,
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //let toml = toml::to_string(&self).unwrap();
        //write!(f, "+++\n{}+++\n{}", toml, self.body)
        let yaml = serde_yaml::to_string(&self).unwrap();
        write!(f, "{}---\n{}", yaml, self.body)
    }
}

impl From<markdown_fm_doc::Document> for Document {
    fn from(item: markdown_fm_doc::Document) -> Self {
        let uuid = UuidB64::new();
        Document {
            id: uuid.to_string(),
            origid: uuid.to_string(),
            authors: vec![item.author],
            body: item.body,
            date: item.date,
            latest: true,
            revision: 1,
            tag: item.tags,
            title: item.title,
            subtitle: item.subtitle,
            filename: item.filename,
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
        let mut s = if self.skip_serializing_body {
            serializer.serialize_struct("Document", 14)?
        } else {
            serializer.serialize_struct("Document", 15)?
        };
        s.serialize_field("authors", &self.authors)?;
        s.serialize_field("date", &self.date)?;
        s.serialize_field("tag", &self.tag)?;
        s.serialize_field("filename", &self.filename)?;
        s.serialize_field("title", &self.title)?;
        if self.subtitle.width() > 0 {
            s.serialize_field("subtitle", &self.subtitle)?;
        };
        s.serialize_field("id", &self.id)?;
        s.serialize_field("origid", &self.origid)?;
        s.serialize_field("weight", &self.weight)?;
        s.serialize_field("revision", &self.revision)?;
        s.serialize_field("latest", &self.latest)?;
        if self.background_img.width() > 0 {
            s.serialize_field("background_img", &self.background_img)?;
        };
        if self.links.len() > 0 {
            s.serialize_field("links", &self.links)?;
        };
        if self.slug.width() > 0 {
            s.serialize_field("slug", &self.slug)?;
        };
        if !self.skip_serializing_body {
            s.serialize_field("body", &self.body)?;
        }
        s.end()
    }
}
