use crate::date::{date_deserializer, Date};
use eyre::Result;
use serde::{de, ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use std::io::{Error, ErrorKind};
use std::str::FromStr;
use std::{fmt, fs, io, marker::PhantomData};
use unicode_width::UnicodeWidthStr;
use uuid_b64::UuidB64;
use yaml_rust::YamlEmitter;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum SerializationType {
    /// Serialize body only when putting into Storage
    Storage,
    Disk,
    Human,
}

impl Default for SerializationType {
    fn default() -> SerializationType {
        SerializationType::Storage
    }
}

// TODO add `backlink` field for hierarchical linking
// TODO add `views` field for counting number of views
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub id: String,
    // If updating an existing doc, this will point to the `id` of the original document, and
    // the revision field should be incremented
    #[serde(default)]
    pub origid: String,
    #[serde(default, alias = "author")]
    pub authors: Vec<String>,
    // Note the custom Serialize implementation below to skip the `body` depending on how
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    #[serde(skip)]
    pub serialization_type: SerializationType,
    /// Epoch seconds
    #[serde(deserialize_with = "date_deserializer")]
    pub date: Date,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub latest: bool,
    #[serde(default)]
    pub revision: u16,
    pub title: String,
    #[serde(default)]
    pub background_img: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    #[serde(deserialize_with = "string_or_list_string", alias = "tag")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub weight: i32,
    #[serde(default)]
    pub views: i32,
    #[serde(default)]
    pub filename: String,
}

#[allow(dead_code)]
fn is_false(v: &bool) -> bool {
    *v
}

impl Document {
    pub fn new() -> Self {
        Document {
            ..Default::default()
        }
    }

    pub fn parse_file(path: &std::path::Path) -> Result<Document, io::Error> {
        let full_path = path.to_str().unwrap();
        let s = fs::read_to_string(full_path)?;

        let (yaml, content) = frontmatter::parse_and_find_content(&s).unwrap();
        match yaml {
            Some(yaml) => {
                let mut out_str = String::new();
                {
                    let mut emitter = YamlEmitter::new(&mut out_str);
                    emitter.dump(&yaml).unwrap(); // dump the YAML object to a String
                }

                let mut doc: Document = match serde_yaml::from_str(&out_str) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error reading yaml {}: {:?} {}", full_path, e, out_str);
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!("Error reading yaml {}: {}", path.display(), e.to_string()),
                        ));
                    }
                };
                doc.filename = String::from(path.file_name().unwrap().to_str().unwrap());
                doc.body = content.to_string();
                if doc.id.width() == 0 {
                    doc.latest = true;
                    let uuid = UuidB64::new();
                    doc.id = uuid.to_string();
                    doc.origid = uuid.to_string();
                }

                Ok(doc)
            }
            None => Err(Error::new(
                ErrorKind::Other,
                format!("Failed to process file {}", path.display()),
            )),
        }
    }
}

/// Support Deserializing a string into a list of string of length 1
fn string_or_list_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec(PhantomData<Vec<String>>);

    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or list of strings")
        }

        // Value is a single string: return a Vec containing that single string
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_owned()])
        }

        fn visit_seq<S>(self, visitor: S) -> Result<Self::Value, S::Error>
        where
            S: de::SeqAccess<'de>,
        {
            Deserialize::deserialize(de::value::SeqAccessDeserializer::new(visitor))
        }
    }

    deserializer.deserialize_any(StringOrVec(PhantomData))
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.serialization_type == SerializationType::Human {
            write!(f, "{}", self.body)
        } else {
            let yaml = serde_yaml::to_string(&self).unwrap();
            write!(f, "{}---\n{}", yaml, self.body)
        }
    }
}

impl From<markdown_fm_doc::Document> for Document {
    fn from(item: markdown_fm_doc::Document) -> Self {
        let uuid = UuidB64::new();
        Document {
            id: uuid.to_string(),
            origid: uuid.to_string(),
            author: vec![item.author],
            body: item.body,
            date: Date::from_str(&item.date).unwrap(),
            latest: true,
            revision: 1,
            tags: item.tags,
            title: item.title,
            subtitle: item.subtitle,
            filename: item.filename,
            ..Default::default()
        }
    }
}

// Custom Serialization to skip various attributes if requested, ie when writing to disk
impl Serialize for Document {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = match self.serialization_type {
            SerializationType::Storage => serializer.serialize_struct("Document", 15)?,
            SerializationType::Disk => serializer.serialize_struct("Document", 13)?,
            SerializationType::Human => {
                return serializer.serialize_struct("Document", 0)?.end();
            }
        };

        s.serialize_field("title", &self.title)?;
        if self.subtitle.width() > 0 {
            s.serialize_field("subtitle", &self.subtitle)?;
        };
        if self.serialization_type == SerializationType::Storage {
            s.serialize_field("date", &self.date)?;
        } else {
            s.serialize_field("date", &format!("{}", &self.date))?;
        }
        s.serialize_field("tags", &self.tags)?;
        if self.serialization_type == SerializationType::Storage {
            s.serialize_field("filename", &self.filename)?;
        };
        s.serialize_field("author", &self.author)?;
        s.serialize_field("id", &self.id)?;
        s.serialize_field("origid", &self.origid)?;
        s.serialize_field("weight", &self.weight)?;
        s.serialize_field("revision", &self.revision)?;
        s.serialize_field("latest", &self.latest)?;
        if self.background_img.width() > 0 {
            s.serialize_field("background_img", &self.background_img)?;
        };
        if !self.links.is_empty() {
            s.serialize_field("links", &self.links)?;
        };
        if self.slug.width() > 0 {
            s.serialize_field("slug", &self.slug)?;
        };
        if self.serialization_type == SerializationType::Storage {
            s.serialize_field("body", &self.body)?;
        }
        s.end()
    }
}
