use chrono::{DateTime, FixedOffset};
use color_eyre::Report;
use eyre::{eyre, Result};
use serde::{de, ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use std::io::{Error, ErrorKind};
use std::{fmt, fs, io, marker::PhantomData};
use unicode_width::UnicodeWidthStr;
use uuid_b64::UuidB64;
use yaml_rust::YamlEmitter;

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
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    #[serde(skip)]
    pub skip_serializing_body: bool,
    /// RFC 3339 based timestamp
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
    #[serde(deserialize_with = "string_or_list_string")]
    pub tag: Vec<String>,
    #[serde(default)]
    pub weight: i32,
    #[serde(default)]
    pub filename: String,
}

impl Document {
    pub fn new() -> Self {
        Document {
            ..Default::default()
        }
    }

    pub fn date_str(&self) -> Result<String, Report> {
        if let Ok(t) = self.parse_date() {
            let ret = t.with_timezone(&chrono::Utc).to_rfc3339();
            return Ok(ret);
        }
        Err(eyre!("❌ Failed to convert path to date '{}'", &self.date))
    }

    pub fn parse_date(&self) -> Result<DateTime<FixedOffset>, Report> {
        if let Ok(rfc3339) = DateTime::parse_from_rfc3339(&self.date) {
            return Ok(rfc3339);
        } else if let Ok(s) = DateTime::parse_from_str(&self.date, &String::from("%Y-%m-%dT%T%z")) {
            return Ok(s);
        }
        eprintln!("❌ Failed to convert path to str");
        Err(eyre!("❌ Failed to convert path to str"))
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

                let mut doc: Document = serde_yaml::from_str(&out_str).unwrap();
                doc.filename = String::from(path.file_name().unwrap().to_str().unwrap());
                doc.body = content.to_string();

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
        if !self.skip_serializing_body {
            s.serialize_field("filename", &self.filename)?;
        };
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
        if !self.links.is_empty() {
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
