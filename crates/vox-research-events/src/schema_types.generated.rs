// @generated — source: contracts/scientia/*.schema.json
// Regenerate: cargo run -p vox-scientia-jsonschema-codegen

// --- contracts/scientia\arxiv-handoff.schema.json ---
pub mod arxiv_handoff_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///Contract for operator-facing arXiv assist handoff bundles exported by vox-publisher.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/arxiv-handoff.schema.json",
    ///  "title": "SCIENTIA arXiv assist handoff",
    ///  "description": "Contract for operator-facing arXiv assist handoff bundles exported by vox-publisher.",
    ///  "type": "object",
    ///  "required": [
    ///    "arxiv_bundle_relpath",
    ///    "body_markdown_relpath",
    ///    "content_sha3_256",
    ///    "main_tex_relpath",
    ///    "note",
    ///    "primary_author",
    ///    "publication_id",
    ///    "schema_version",
    ///    "staging_checksums_relpath",
    ///    "staging_generated_by",
    ///    "title",
    ///    "workflow"
    ///  ],
    ///  "properties": {
    ///    "arxiv_bundle_relpath": {
    ///      "type": "string",
    ///      "const": "arxiv_bundle.tar.gz"
    ///    },
    ///    "body_markdown_relpath": {
    ///      "type": "string",
    ///      "const": "body.md"
    ///    },
    ///    "content_sha3_256": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "main_tex_relpath": {
    ///      "type": "string",
    ///      "const": "main.tex"
    ///    },
    ///    "note": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "primary_author": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "publication_id": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "schema_version": {
    ///      "type": "integer",
    ///      "const": 1
    ///    },
    ///    "staging_checksums_relpath": {
    ///      "type": "string",
    ///      "const": "staging_checksums.json"
    ///    },
    ///    "staging_generated_by": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "title": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "workflow": {
    ///      "type": "string",
    ///      "const": "arxiv_operator_assist"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaArXivAssistHandoff {
        pub arxiv_bundle_relpath: ::std::string::String,
        pub body_markdown_relpath: ::std::string::String,
        pub content_sha3_256: ScientiaArXivAssistHandoffContentSha3256,
        pub main_tex_relpath: ::std::string::String,
        pub note: ScientiaArXivAssistHandoffNote,
        pub primary_author: ScientiaArXivAssistHandoffPrimaryAuthor,
        pub publication_id: ScientiaArXivAssistHandoffPublicationId,
        pub schema_version: i64,
        pub staging_checksums_relpath: ::std::string::String,
        pub staging_generated_by: ScientiaArXivAssistHandoffStagingGeneratedBy,
        pub title: ScientiaArXivAssistHandoffTitle,
        pub workflow: ::std::string::String,
    }
    ///`ScientiaArXivAssistHandoffContentSha3256`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaArXivAssistHandoffContentSha3256(::std::string::String);
    impl ::std::ops::Deref for ScientiaArXivAssistHandoffContentSha3256 {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaArXivAssistHandoffContentSha3256>
    for ::std::string::String {
        fn from(value: ScientiaArXivAssistHandoffContentSha3256) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaArXivAssistHandoffContentSha3256 {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaArXivAssistHandoffContentSha3256 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaArXivAssistHandoffContentSha3256 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaArXivAssistHandoffContentSha3256 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaArXivAssistHandoffContentSha3256 {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaArXivAssistHandoffNote`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaArXivAssistHandoffNote(::std::string::String);
    impl ::std::ops::Deref for ScientiaArXivAssistHandoffNote {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaArXivAssistHandoffNote> for ::std::string::String {
        fn from(value: ScientiaArXivAssistHandoffNote) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaArXivAssistHandoffNote {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaArXivAssistHandoffNote {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for ScientiaArXivAssistHandoffNote {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ScientiaArXivAssistHandoffNote {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaArXivAssistHandoffNote {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaArXivAssistHandoffPrimaryAuthor`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaArXivAssistHandoffPrimaryAuthor(::std::string::String);
    impl ::std::ops::Deref for ScientiaArXivAssistHandoffPrimaryAuthor {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaArXivAssistHandoffPrimaryAuthor>
    for ::std::string::String {
        fn from(value: ScientiaArXivAssistHandoffPrimaryAuthor) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaArXivAssistHandoffPrimaryAuthor {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaArXivAssistHandoffPrimaryAuthor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaArXivAssistHandoffPrimaryAuthor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaArXivAssistHandoffPrimaryAuthor {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaArXivAssistHandoffPrimaryAuthor {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaArXivAssistHandoffPublicationId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaArXivAssistHandoffPublicationId(::std::string::String);
    impl ::std::ops::Deref for ScientiaArXivAssistHandoffPublicationId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaArXivAssistHandoffPublicationId>
    for ::std::string::String {
        fn from(value: ScientiaArXivAssistHandoffPublicationId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaArXivAssistHandoffPublicationId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaArXivAssistHandoffPublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaArXivAssistHandoffPublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaArXivAssistHandoffPublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaArXivAssistHandoffPublicationId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaArXivAssistHandoffStagingGeneratedBy`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaArXivAssistHandoffStagingGeneratedBy(::std::string::String);
    impl ::std::ops::Deref for ScientiaArXivAssistHandoffStagingGeneratedBy {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaArXivAssistHandoffStagingGeneratedBy>
    for ::std::string::String {
        fn from(value: ScientiaArXivAssistHandoffStagingGeneratedBy) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaArXivAssistHandoffStagingGeneratedBy {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaArXivAssistHandoffStagingGeneratedBy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaArXivAssistHandoffStagingGeneratedBy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaArXivAssistHandoffStagingGeneratedBy {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaArXivAssistHandoffStagingGeneratedBy {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaArXivAssistHandoffTitle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaArXivAssistHandoffTitle(::std::string::String);
    impl ::std::ops::Deref for ScientiaArXivAssistHandoffTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaArXivAssistHandoffTitle> for ::std::string::String {
        fn from(value: ScientiaArXivAssistHandoffTitle) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaArXivAssistHandoffTitle {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaArXivAssistHandoffTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaArXivAssistHandoffTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ScientiaArXivAssistHandoffTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaArXivAssistHandoffTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
}

// --- contracts/scientia\canonical-publication-metadata.v1.schema.json ---
pub mod canonical_publication_metadata_v1_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///Single-source manifest-centered metadata graph used for route-specific transformation (Crossref, DataCite, Zenodo, arXiv handoff, OpenReview, syndication).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/canonical-publication-metadata.v1.schema.json",
    ///  "title": "SCIENTIA canonical publication metadata v1",
    ///  "description": "Single-source manifest-centered metadata graph used for route-specific transformation (Crossref, DataCite, Zenodo, arXiv handoff, OpenReview, syndication).",
    ///  "type": "object",
    ///  "required": [
    ///    "contributors",
    ///    "distribution",
    ///    "identity",
    ///    "policy",
    ///    "provenance",
    ///    "rights_and_funding",
    ///    "version"
    ///  ],
    ///  "properties": {
    ///    "contributors": {
    ///      "type": "object",
    ///      "required": [
    ///        "authors"
    ///      ],
    ///      "properties": {
    ///        "authors": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "object",
    ///            "required": [
    ///              "name"
    ///            ],
    ///            "properties": {
    ///              "affiliation": {
    ///                "type": "object",
    ///                "required": [
    ///                  "name"
    ///                ],
    ///                "properties": {
    ///                  "name": {
    ///                    "type": "string",
    ///                    "minLength": 1
    ///                  },
    ///                  "ror": {
    ///                    "type": "string"
    ///                  }
    ///                },
    ///                "additionalProperties": false
    ///              },
    ///              "name": {
    ///                "type": "string",
    ///                "minLength": 1
    ///              },
    ///              "orcid": {
    ///                "type": "string"
    ///              },
    ///              "roles": {
    ///                "type": "array",
    ///                "items": {
    ///                  "type": "string"
    ///                }
    ///              }
    ///            },
    ///            "additionalProperties": false
    ///          },
    ///          "minItems": 1
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "distribution": {
    ///      "type": "object",
    ///      "required": [
    ///        "routes"
    ///      ],
    ///      "properties": {
    ///        "canonical_repo_url": {
    ///          "type": "string"
    ///        },
    ///        "distribution_policy_ref": {
    ///          "type": "string"
    ///        },
    ///        "embargo_lift_utc": {
    ///          "type": "string",
    ///          "format": "date-time"
    ///        },
    ///        "og_image_url": {
    ///          "type": "string"
    ///        },
    ///        "preferred_citation": {
    ///          "type": "string"
    ///        },
    ///        "primary_doi": {
    ///          "type": "string"
    ///        },
    ///        "routes": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "enum": [
    ///              "crossref",
    ///              "datacite",
    ///              "zenodo",
    ///              "arxiv_handoff",
    ///              "openreview",
    ///              "social"
    ///            ]
    ///          }
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "evidence": {
    ///      "type": "object"
    ///    },
    ///    "identity": {
    ///      "type": "object",
    ///      "required": [
    ///        "abstract",
    ///        "keywords",
    ///        "target_profile",
    ///        "title"
    ///      ],
    ///      "properties": {
    ///        "abstract": {
    ///          "type": "string",
    ///          "minLength": 1
    ///        },
    ///        "keywords": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "target_profile": {
    ///          "type": "string",
    ///          "enum": [
    ///            "journal",
    ///            "preprint",
    ///            "repository",
    ///            "social"
    ///          ]
    ///        },
    ///        "title": {
    ///          "type": "string",
    ///          "minLength": 1
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "policy": {
    ///      "type": "object",
    ///      "required": [
    ///        "ai_disclosure"
    ///      ],
    ///      "properties": {
    ///        "ai_disclosure": {
    ///          "type": "object",
    ///          "required": [
    ///            "declared"
    ///          ],
    ///          "properties": {
    ///            "declared": {
    ///              "type": "boolean"
    ///            },
    ///            "tools": {
    ///              "type": "array",
    ///              "items": {
    ///                "type": "object",
    ///                "required": [
    ///                  "name"
    ///                ],
    ///                "properties": {
    ///                  "name": {
    ///                    "type": "string"
    ///                  },
    ///                  "scope": {
    ///                    "type": "string"
    ///                  },
    ///                  "version": {
    ///                    "type": "string"
    ///                  }
    ///                },
    ///                "additionalProperties": false
    ///              }
    ///            }
    ///          },
    ///          "additionalProperties": false
    ///        },
    ///        "broader_impact_statement": {
    ///          "type": "string"
    ///        },
    ///        "double_blind_ready": {
    ///          "type": "boolean"
    ///        },
    ///        "ethics_statement": {
    ///          "type": "string"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "provenance": {
    ///      "type": "object",
    ///      "required": [
    ///        "manifest_digest",
    ///        "publication_id"
    ///      ],
    ///      "properties": {
    ///        "commit_sha": {
    ///          "type": "string"
    ///        },
    ///        "corrects_publication_id": {
    ///          "type": "string"
    ///        },
    ///        "evidence_pack_digest": {
    ///          "type": "string"
    ///        },
    ///        "manifest_digest": {
    ///          "type": "string",
    ///          "minLength": 16
    ///        },
    ///        "publication_id": {
    ///          "type": "string",
    ///          "minLength": 1
    ///        },
    ///        "repository_id": {
    ///          "type": "string"
    ///        },
    ///        "supersedes_publication_id": {
    ///          "type": "string"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "rights_and_funding": {
    ///      "type": "object",
    ///      "properties": {
    ///        "conflict_of_interest": {
    ///          "type": "string"
    ///        },
    ///        "funding": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "object",
    ///            "required": [
    ///              "name"
    ///            ],
    ///            "properties": {
    ///              "award_number": {
    ///                "type": "string"
    ///              },
    ///              "funder_id": {
    ///                "type": "string"
    ///              },
    ///              "name": {
    ///                "type": "string"
    ///              }
    ///            },
    ///            "additionalProperties": false
    ///          }
    ///        },
    ///        "license": {
    ///          "type": "string"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "schema_version": {
    ///      "type": "string",
    ///      "enum": [
    ///        "1.0",
    ///        "1.1"
    ///      ]
    ///    },
    ///    "version": {
    ///      "type": "string",
    ///      "const": "v1"
    ///    }
    ///  },
    ///  "additionalProperties": false,
    ///  "x-vox-version": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1 {
        pub contributors: ScientiaCanonicalPublicationMetadataV1Contributors,
        pub distribution: ScientiaCanonicalPublicationMetadataV1Distribution,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub evidence: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        pub identity: ScientiaCanonicalPublicationMetadataV1Identity,
        pub policy: ScientiaCanonicalPublicationMetadataV1Policy,
        pub provenance: ScientiaCanonicalPublicationMetadataV1Provenance,
        pub rights_and_funding: ScientiaCanonicalPublicationMetadataV1RightsAndFunding,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub schema_version: ::std::option::Option<
            ScientiaCanonicalPublicationMetadataV1SchemaVersion,
        >,
        pub version: ::std::string::String,
    }
    ///`ScientiaCanonicalPublicationMetadataV1Contributors`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "authors"
    ///  ],
    ///  "properties": {
    ///    "authors": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "name"
    ///        ],
    ///        "properties": {
    ///          "affiliation": {
    ///            "type": "object",
    ///            "required": [
    ///              "name"
    ///            ],
    ///            "properties": {
    ///              "name": {
    ///                "type": "string",
    ///                "minLength": 1
    ///              },
    ///              "ror": {
    ///                "type": "string"
    ///              }
    ///            },
    ///            "additionalProperties": false
    ///          },
    ///          "name": {
    ///            "type": "string",
    ///            "minLength": 1
    ///          },
    ///          "orcid": {
    ///            "type": "string"
    ///          },
    ///          "roles": {
    ///            "type": "array",
    ///            "items": {
    ///              "type": "string"
    ///            }
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      },
    ///      "minItems": 1
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1Contributors {
        pub authors: ::std::vec::Vec<
            ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItem,
        >,
    }
    ///`ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "affiliation": {
    ///      "type": "object",
    ///      "required": [
    ///        "name"
    ///      ],
    ///      "properties": {
    ///        "name": {
    ///          "type": "string",
    ///          "minLength": 1
    ///        },
    ///        "ror": {
    ///          "type": "string"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "name": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "orcid": {
    ///      "type": "string"
    ///    },
    ///    "roles": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub affiliation: ::std::option::Option<
            ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliation,
        >,
        pub name: ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub orcid: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub roles: ::std::vec::Vec<::std::string::String>,
    }
    ///`ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliation`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "ror": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliation {
        pub name: ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ror: ::std::option::Option<::std::string::String>,
    }
    ///`ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<
        ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName,
    > for ::std::string::String {
        fn from(
            value: ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemAffiliationName {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<
        ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName,
    > for ::std::string::String {
        fn from(
            value: ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaCanonicalPublicationMetadataV1ContributorsAuthorsItemName {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaCanonicalPublicationMetadataV1Distribution`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "routes"
    ///  ],
    ///  "properties": {
    ///    "canonical_repo_url": {
    ///      "type": "string"
    ///    },
    ///    "distribution_policy_ref": {
    ///      "type": "string"
    ///    },
    ///    "embargo_lift_utc": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "og_image_url": {
    ///      "type": "string"
    ///    },
    ///    "preferred_citation": {
    ///      "type": "string"
    ///    },
    ///    "primary_doi": {
    ///      "type": "string"
    ///    },
    ///    "routes": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "enum": [
    ///          "crossref",
    ///          "datacite",
    ///          "zenodo",
    ///          "arxiv_handoff",
    ///          "openreview",
    ///          "social"
    ///        ]
    ///      }
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1Distribution {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub canonical_repo_url: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub distribution_policy_ref: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub embargo_lift_utc: ::std::option::Option<
            ::chrono::DateTime<::chrono::offset::Utc>,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub og_image_url: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub preferred_citation: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub primary_doi: ::std::option::Option<::std::string::String>,
        pub routes: ::std::vec::Vec<
            ScientiaCanonicalPublicationMetadataV1DistributionRoutesItem,
        >,
    }
    ///`ScientiaCanonicalPublicationMetadataV1DistributionRoutesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "crossref",
    ///    "datacite",
    ///    "zenodo",
    ///    "arxiv_handoff",
    ///    "openreview",
    ///    "social"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaCanonicalPublicationMetadataV1DistributionRoutesItem {
        #[serde(rename = "crossref")]
        Crossref,
        #[serde(rename = "datacite")]
        Datacite,
        #[serde(rename = "zenodo")]
        Zenodo,
        #[serde(rename = "arxiv_handoff")]
        ArxivHandoff,
        #[serde(rename = "openreview")]
        Openreview,
        #[serde(rename = "social")]
        Social,
    }
    impl ::std::fmt::Display
    for ScientiaCanonicalPublicationMetadataV1DistributionRoutesItem {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Crossref => f.write_str("crossref"),
                Self::Datacite => f.write_str("datacite"),
                Self::Zenodo => f.write_str("zenodo"),
                Self::ArxivHandoff => f.write_str("arxiv_handoff"),
                Self::Openreview => f.write_str("openreview"),
                Self::Social => f.write_str("social"),
            }
        }
    }
    impl ::std::str::FromStr
    for ScientiaCanonicalPublicationMetadataV1DistributionRoutesItem {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "crossref" => Ok(Self::Crossref),
                "datacite" => Ok(Self::Datacite),
                "zenodo" => Ok(Self::Zenodo),
                "arxiv_handoff" => Ok(Self::ArxivHandoff),
                "openreview" => Ok(Self::Openreview),
                "social" => Ok(Self::Social),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaCanonicalPublicationMetadataV1DistributionRoutesItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1DistributionRoutesItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1DistributionRoutesItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaCanonicalPublicationMetadataV1Identity`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "abstract",
    ///    "keywords",
    ///    "target_profile",
    ///    "title"
    ///  ],
    ///  "properties": {
    ///    "abstract": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "keywords": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "target_profile": {
    ///      "type": "string",
    ///      "enum": [
    ///        "journal",
    ///        "preprint",
    ///        "repository",
    ///        "social"
    ///      ]
    ///    },
    ///    "title": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1Identity {
        #[serde(rename = "abstract")]
        pub abstract_: ScientiaCanonicalPublicationMetadataV1IdentityAbstract,
        pub keywords: ::std::vec::Vec<::std::string::String>,
        pub target_profile: ScientiaCanonicalPublicationMetadataV1IdentityTargetProfile,
        pub title: ScientiaCanonicalPublicationMetadataV1IdentityTitle,
    }
    ///`ScientiaCanonicalPublicationMetadataV1IdentityAbstract`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaCanonicalPublicationMetadataV1IdentityAbstract(::std::string::String);
    impl ::std::ops::Deref for ScientiaCanonicalPublicationMetadataV1IdentityAbstract {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaCanonicalPublicationMetadataV1IdentityAbstract>
    for ::std::string::String {
        fn from(value: ScientiaCanonicalPublicationMetadataV1IdentityAbstract) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaCanonicalPublicationMetadataV1IdentityAbstract {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaCanonicalPublicationMetadataV1IdentityAbstract {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1IdentityAbstract {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1IdentityAbstract {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaCanonicalPublicationMetadataV1IdentityAbstract {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaCanonicalPublicationMetadataV1IdentityTargetProfile`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "journal",
    ///    "preprint",
    ///    "repository",
    ///    "social"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaCanonicalPublicationMetadataV1IdentityTargetProfile {
        #[serde(rename = "journal")]
        Journal,
        #[serde(rename = "preprint")]
        Preprint,
        #[serde(rename = "repository")]
        Repository,
        #[serde(rename = "social")]
        Social,
    }
    impl ::std::fmt::Display
    for ScientiaCanonicalPublicationMetadataV1IdentityTargetProfile {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Journal => f.write_str("journal"),
                Self::Preprint => f.write_str("preprint"),
                Self::Repository => f.write_str("repository"),
                Self::Social => f.write_str("social"),
            }
        }
    }
    impl ::std::str::FromStr
    for ScientiaCanonicalPublicationMetadataV1IdentityTargetProfile {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "journal" => Ok(Self::Journal),
                "preprint" => Ok(Self::Preprint),
                "repository" => Ok(Self::Repository),
                "social" => Ok(Self::Social),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaCanonicalPublicationMetadataV1IdentityTargetProfile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1IdentityTargetProfile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1IdentityTargetProfile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaCanonicalPublicationMetadataV1IdentityTitle`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaCanonicalPublicationMetadataV1IdentityTitle(::std::string::String);
    impl ::std::ops::Deref for ScientiaCanonicalPublicationMetadataV1IdentityTitle {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaCanonicalPublicationMetadataV1IdentityTitle>
    for ::std::string::String {
        fn from(value: ScientiaCanonicalPublicationMetadataV1IdentityTitle) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaCanonicalPublicationMetadataV1IdentityTitle {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaCanonicalPublicationMetadataV1IdentityTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1IdentityTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1IdentityTitle {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaCanonicalPublicationMetadataV1IdentityTitle {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaCanonicalPublicationMetadataV1Policy`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "ai_disclosure"
    ///  ],
    ///  "properties": {
    ///    "ai_disclosure": {
    ///      "type": "object",
    ///      "required": [
    ///        "declared"
    ///      ],
    ///      "properties": {
    ///        "declared": {
    ///          "type": "boolean"
    ///        },
    ///        "tools": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "object",
    ///            "required": [
    ///              "name"
    ///            ],
    ///            "properties": {
    ///              "name": {
    ///                "type": "string"
    ///              },
    ///              "scope": {
    ///                "type": "string"
    ///              },
    ///              "version": {
    ///                "type": "string"
    ///              }
    ///            },
    ///            "additionalProperties": false
    ///          }
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "broader_impact_statement": {
    ///      "type": "string"
    ///    },
    ///    "double_blind_ready": {
    ///      "type": "boolean"
    ///    },
    ///    "ethics_statement": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1Policy {
        pub ai_disclosure: ScientiaCanonicalPublicationMetadataV1PolicyAiDisclosure,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub broader_impact_statement: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub double_blind_ready: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub ethics_statement: ::std::option::Option<::std::string::String>,
    }
    ///`ScientiaCanonicalPublicationMetadataV1PolicyAiDisclosure`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "declared"
    ///  ],
    ///  "properties": {
    ///    "declared": {
    ///      "type": "boolean"
    ///    },
    ///    "tools": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "name"
    ///        ],
    ///        "properties": {
    ///          "name": {
    ///            "type": "string"
    ///          },
    ///          "scope": {
    ///            "type": "string"
    ///          },
    ///          "version": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1PolicyAiDisclosure {
        pub declared: bool,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tools: ::std::vec::Vec<
            ScientiaCanonicalPublicationMetadataV1PolicyAiDisclosureToolsItem,
        >,
    }
    ///`ScientiaCanonicalPublicationMetadataV1PolicyAiDisclosureToolsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "scope": {
    ///      "type": "string"
    ///    },
    ///    "version": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1PolicyAiDisclosureToolsItem {
        pub name: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub scope: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub version: ::std::option::Option<::std::string::String>,
    }
    ///`ScientiaCanonicalPublicationMetadataV1Provenance`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "manifest_digest",
    ///    "publication_id"
    ///  ],
    ///  "properties": {
    ///    "commit_sha": {
    ///      "type": "string"
    ///    },
    ///    "corrects_publication_id": {
    ///      "type": "string"
    ///    },
    ///    "evidence_pack_digest": {
    ///      "type": "string"
    ///    },
    ///    "manifest_digest": {
    ///      "type": "string",
    ///      "minLength": 16
    ///    },
    ///    "publication_id": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "repository_id": {
    ///      "type": "string"
    ///    },
    ///    "supersedes_publication_id": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1Provenance {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub commit_sha: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub corrects_publication_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub evidence_pack_digest: ::std::option::Option<::std::string::String>,
        pub manifest_digest: ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest,
        pub publication_id: ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub repository_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub supersedes_publication_id: ::std::option::Option<::std::string::String>,
    }
    ///`ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 16
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest>
    for ::std::string::String {
        fn from(
            value: ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 16usize {
                return Err("shorter than 16 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaCanonicalPublicationMetadataV1ProvenanceManifestDigest {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId>
    for ::std::string::String {
        fn from(
            value: ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaCanonicalPublicationMetadataV1ProvenancePublicationId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaCanonicalPublicationMetadataV1RightsAndFunding`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "conflict_of_interest": {
    ///      "type": "string"
    ///    },
    ///    "funding": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "name"
    ///        ],
    ///        "properties": {
    ///          "award_number": {
    ///            "type": "string"
    ///          },
    ///          "funder_id": {
    ///            "type": "string"
    ///          },
    ///          "name": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "license": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1RightsAndFunding {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub conflict_of_interest: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub funding: ::std::vec::Vec<
            ScientiaCanonicalPublicationMetadataV1RightsAndFundingFundingItem,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub license: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ScientiaCanonicalPublicationMetadataV1RightsAndFunding {
        fn default() -> Self {
            Self {
                conflict_of_interest: Default::default(),
                funding: Default::default(),
                license: Default::default(),
            }
        }
    }
    ///`ScientiaCanonicalPublicationMetadataV1RightsAndFundingFundingItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "name"
    ///  ],
    ///  "properties": {
    ///    "award_number": {
    ///      "type": "string"
    ///    },
    ///    "funder_id": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaCanonicalPublicationMetadataV1RightsAndFundingFundingItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub award_number: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub funder_id: ::std::option::Option<::std::string::String>,
        pub name: ::std::string::String,
    }
    ///`ScientiaCanonicalPublicationMetadataV1SchemaVersion`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "1.0",
    ///    "1.1"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaCanonicalPublicationMetadataV1SchemaVersion {
        #[serde(rename = "1.0")]
        X10,
        #[serde(rename = "1.1")]
        X11,
    }
    impl ::std::fmt::Display for ScientiaCanonicalPublicationMetadataV1SchemaVersion {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X10 => f.write_str("1.0"),
                Self::X11 => f.write_str("1.1"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaCanonicalPublicationMetadataV1SchemaVersion {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "1.0" => Ok(Self::X10),
                "1.1" => Ok(Self::X11),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaCanonicalPublicationMetadataV1SchemaVersion {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1SchemaVersion {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaCanonicalPublicationMetadataV1SchemaVersion {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
}

// --- contracts/scientia\discovery-signal.schema.json ---
pub mod discovery_signal_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///`ScientiaDiscoverySignal`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/discovery-signal.schema.json",
    ///  "title": "SCIENTIA discovery signal",
    ///  "type": "object",
    ///  "required": [
    ///    "code",
    ///    "family",
    ///    "provenance",
    ///    "strength",
    ///    "summary"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "family": {
    ///      "type": "string",
    ///      "enum": [
    ///        "unspecified",
    ///        "eval_gate",
    ///        "benchmark_pair",
    ///        "documentation",
    ///        "telemetry_aggregate",
    ///        "operator_attestation",
    ///        "mens_scorecard",
    ///        "trust_rollup",
    ///        "reproducibility_artifact",
    ///        "linked_corpus",
    ///        "finding_candidate_signal"
    ///      ]
    ///    },
    ///    "provenance": {
    ///      "type": "object",
    ///      "properties": {
    ///        "digest": {
    ///          "type": "string"
    ///        },
    ///        "metric_type": {
    ///          "type": "string"
    ///        },
    ///        "origin": {
    ///          "type": "string"
    ///        },
    ///        "recorded_at_ms": {
    ///          "type": "integer"
    ///        },
    ///        "repo_path": {
    ///          "type": "string"
    ///        },
    ///        "run_id": {
    ///          "type": "string"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "source_ref": {
    ///      "type": "string"
    ///    },
    ///    "strength": {
    ///      "type": "string",
    ///      "enum": [
    ///        "supporting",
    ///        "strong",
    ///        "informational"
    ///      ]
    ///    },
    ///    "summary": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDiscoverySignal {
        pub code: ScientiaDiscoverySignalCode,
        pub family: ScientiaDiscoverySignalFamily,
        pub provenance: ScientiaDiscoverySignalProvenance,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source_ref: ::std::option::Option<::std::string::String>,
        pub strength: ScientiaDiscoverySignalStrength,
        pub summary: ::std::string::String,
    }
    ///`ScientiaDiscoverySignalCode`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaDiscoverySignalCode(::std::string::String);
    impl ::std::ops::Deref for ScientiaDiscoverySignalCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaDiscoverySignalCode> for ::std::string::String {
        fn from(value: ScientiaDiscoverySignalCode) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaDiscoverySignalCode {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaDiscoverySignalCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for ScientiaDiscoverySignalCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ScientiaDiscoverySignalCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaDiscoverySignalCode {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaDiscoverySignalFamily`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "unspecified",
    ///    "eval_gate",
    ///    "benchmark_pair",
    ///    "documentation",
    ///    "telemetry_aggregate",
    ///    "operator_attestation",
    ///    "mens_scorecard",
    ///    "trust_rollup",
    ///    "reproducibility_artifact",
    ///    "linked_corpus",
    ///    "finding_candidate_signal"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaDiscoverySignalFamily {
        #[serde(rename = "unspecified")]
        Unspecified,
        #[serde(rename = "eval_gate")]
        EvalGate,
        #[serde(rename = "benchmark_pair")]
        BenchmarkPair,
        #[serde(rename = "documentation")]
        Documentation,
        #[serde(rename = "telemetry_aggregate")]
        TelemetryAggregate,
        #[serde(rename = "operator_attestation")]
        OperatorAttestation,
        #[serde(rename = "mens_scorecard")]
        MensScorecard,
        #[serde(rename = "trust_rollup")]
        TrustRollup,
        #[serde(rename = "reproducibility_artifact")]
        ReproducibilityArtifact,
        #[serde(rename = "linked_corpus")]
        LinkedCorpus,
        #[serde(rename = "finding_candidate_signal")]
        FindingCandidateSignal,
    }
    impl ::std::fmt::Display for ScientiaDiscoverySignalFamily {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Unspecified => f.write_str("unspecified"),
                Self::EvalGate => f.write_str("eval_gate"),
                Self::BenchmarkPair => f.write_str("benchmark_pair"),
                Self::Documentation => f.write_str("documentation"),
                Self::TelemetryAggregate => f.write_str("telemetry_aggregate"),
                Self::OperatorAttestation => f.write_str("operator_attestation"),
                Self::MensScorecard => f.write_str("mens_scorecard"),
                Self::TrustRollup => f.write_str("trust_rollup"),
                Self::ReproducibilityArtifact => f.write_str("reproducibility_artifact"),
                Self::LinkedCorpus => f.write_str("linked_corpus"),
                Self::FindingCandidateSignal => f.write_str("finding_candidate_signal"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaDiscoverySignalFamily {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "unspecified" => Ok(Self::Unspecified),
                "eval_gate" => Ok(Self::EvalGate),
                "benchmark_pair" => Ok(Self::BenchmarkPair),
                "documentation" => Ok(Self::Documentation),
                "telemetry_aggregate" => Ok(Self::TelemetryAggregate),
                "operator_attestation" => Ok(Self::OperatorAttestation),
                "mens_scorecard" => Ok(Self::MensScorecard),
                "trust_rollup" => Ok(Self::TrustRollup),
                "reproducibility_artifact" => Ok(Self::ReproducibilityArtifact),
                "linked_corpus" => Ok(Self::LinkedCorpus),
                "finding_candidate_signal" => Ok(Self::FindingCandidateSignal),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaDiscoverySignalFamily {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for ScientiaDiscoverySignalFamily {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ScientiaDiscoverySignalFamily {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaDiscoverySignalProvenance`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "digest": {
    ///      "type": "string"
    ///    },
    ///    "metric_type": {
    ///      "type": "string"
    ///    },
    ///    "origin": {
    ///      "type": "string"
    ///    },
    ///    "recorded_at_ms": {
    ///      "type": "integer"
    ///    },
    ///    "repo_path": {
    ///      "type": "string"
    ///    },
    ///    "run_id": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDiscoverySignalProvenance {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub digest: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub metric_type: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub origin: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub recorded_at_ms: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub repo_path: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub run_id: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ScientiaDiscoverySignalProvenance {
        fn default() -> Self {
            Self {
                digest: Default::default(),
                metric_type: Default::default(),
                origin: Default::default(),
                recorded_at_ms: Default::default(),
                repo_path: Default::default(),
                run_id: Default::default(),
            }
        }
    }
    ///`ScientiaDiscoverySignalStrength`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "supporting",
    ///    "strong",
    ///    "informational"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaDiscoverySignalStrength {
        #[serde(rename = "supporting")]
        Supporting,
        #[serde(rename = "strong")]
        Strong,
        #[serde(rename = "informational")]
        Informational,
    }
    impl ::std::fmt::Display for ScientiaDiscoverySignalStrength {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Supporting => f.write_str("supporting"),
                Self::Strong => f.write_str("strong"),
                Self::Informational => f.write_str("informational"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaDiscoverySignalStrength {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "supporting" => Ok(Self::Supporting),
                "strong" => Ok(Self::Strong),
                "informational" => Ok(Self::Informational),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaDiscoverySignalStrength {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDiscoverySignalStrength {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ScientiaDiscoverySignalStrength {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
}

// --- contracts/scientia\distribution.schema.json ---
pub mod distribution_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///Syndication intent for Scientia items. Runtime: embed this object under the `syndication` key in publication `metadata_json`, or under `syndication` in markdown/DB-derived metadata. Keys match `SyndicationConfig` in `vox-publisher` (not the legacy `scientia_distribution` root name).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox/contracts/scientia/distribution.schema.json",
    ///  "title": "ScientiaDistribution",
    ///  "description": "Syndication intent for Scientia items. Runtime: embed this object under the `syndication` key in publication `metadata_json`, or under `syndication` in markdown/DB-derived metadata. Keys match `SyndicationConfig` in `vox-publisher` (not the legacy `scientia_distribution` root name).",
    ///  "type": "object",
    ///  "properties": {
    ///    "channel_payloads": {
    ///      "type": "object",
    ///      "properties": {
    ///        "crates_io": {
    ///          "description": "Contract placeholder only: `vox-publisher` does not perform crates.io releases yet. If enabled in routing, outcomes are explicit dry-run or not-implemented failures—never silent success.",
    ///          "type": "object",
    ///          "required": [
    ///            "crates_to_update"
    ///          ],
    ///          "properties": {
    ///            "crates_to_update": {
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string",
    ///                "minLength": 1
    ///              }
    ///            }
    ///          },
    ///          "additionalProperties": false
    ///        },
    ///        "github": {
    ///          "type": "object",
    ///          "properties": {
    ///            "discussion_category": {
    ///              "type": "string"
    ///            },
    ///            "draft": {
    ///              "type": "boolean"
    ///            },
    ///            "post_type": {
    ///              "type": "string",
    ///              "enum": [
    ///                "Release",
    ///                "Discussion"
    ///              ]
    ///            },
    ///            "release_tag": {
    ///              "type": "string"
    ///            },
    ///            "repo": {
    ///              "type": "string",
    ///              "minLength": 1
    ///            }
    ///          },
    ///          "additionalProperties": false
    ///        },
    ///        "hacker_news": {
    ///          "type": "object",
    ///          "properties": {
    ///            "mode": {
    ///              "type": "string",
    ///              "enum": [
    ///                "manual_assist"
    ///              ]
    ///            },
    ///            "title_override": {
    ///              "type": "string"
    ///            },
    ///            "url_override": {
    ///              "type": "string"
    ///            }
    ///          },
    ///          "additionalProperties": false
    ///        },
    ///        "open_collective": {
    ///          "type": "object",
    ///          "required": [
    ///            "collective_slug"
    ///          ],
    ///          "properties": {
    ///            "collective_slug": {
    ///              "type": "string",
    ///              "minLength": 1
    ///            },
    ///            "is_private": {
    ///              "type": "boolean"
    ///            }
    ///          },
    ///          "additionalProperties": false
    ///        },
    ///        "reddit": {
    ///          "type": "object",
    ///          "required": [
    ///            "subreddit"
    ///          ],
    ///          "properties": {
    ///            "kind": {
    ///              "type": "string",
    ///              "enum": [
    ///                "link",
    ///                "self_post"
    ///              ]
    ///            },
    ///            "nsfw": {
    ///              "type": "boolean"
    ///            },
    ///            "send_replies": {
    ///              "type": "boolean"
    ///            },
    ///            "spoiler": {
    ///              "type": "boolean"
    ///            },
    ///            "subreddit": {
    ///              "type": "string",
    ///              "minLength": 1
    ///            },
    ///            "text_override": {
    ///              "type": "string"
    ///            },
    ///            "title_override": {
    ///              "type": "string"
    ///            },
    ///            "url_override": {
    ///              "type": "string"
    ///            }
    ///          },
    ///          "additionalProperties": false
    ///        },
    ///        "twitter": {
    ///          "type": "object",
    ///          "properties": {
    ///            "short_text": {
    ///              "type": "string"
    ///            },
    ///            "thread": {
    ///              "type": "boolean"
    ///            }
    ///          },
    ///          "additionalProperties": false
    ///        },
    ///        "youtube": {
    ///          "type": "object",
    ///          "required": [
    ///            "video_asset_ref"
    ///          ],
    ///          "properties": {
    ///            "category_id": {
    ///              "type": "string"
    ///            },
    ///            "description_override": {
    ///              "type": "string"
    ///            },
    ///            "notify_subscribers": {
    ///              "type": "boolean"
    ///            },
    ///            "privacy_status": {
    ///              "type": "string",
    ///              "enum": [
    ///                "private",
    ///                "unlisted",
    ///                "public"
    ///              ]
    ///            },
    ///            "tags": {
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string"
    ///              }
    ///            },
    ///            "title_override": {
    ///              "type": "string"
    ///            },
    ///            "video_asset_ref": {
    ///              "type": "string",
    ///              "minLength": 1
    ///            }
    ///          },
    ///          "additionalProperties": false
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "channels": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "enum": [
    ///          "rss",
    ///          "twitter",
    ///          "github",
    ///          "open_collective",
    ///          "reddit",
    ///          "hacker_news",
    ///          "youtube",
    ///          "crates_io"
    ///        ]
    ///      }
    ///    },
    ///    "distribution_policy": {
    ///      "type": "object",
    ///      "properties": {
    ///        "approval_required": {
    ///          "type": "boolean"
    ///        },
    ///        "channel_policy": {
    ///          "type": "object",
    ///          "additionalProperties": {
    ///            "type": "object",
    ///            "properties": {
    ///              "enabled": {
    ///                "type": "boolean"
    ///              },
    ///              "template_profile": {
    ///                "type": "string"
    ///              },
    ///              "topic_filters": {
    ///                "type": "object",
    ///                "properties": {
    ///                  "exclude_tags": {
    ///                    "type": "array",
    ///                    "items": {
    ///                      "type": "string"
    ///                    }
    ///                  },
    ///                  "include_tags": {
    ///                    "type": "array",
    ///                    "items": {
    ///                      "type": "string"
    ///                    }
    ///                  },
    ///                  "min_topic_score": {
    ///                    "type": "number",
    ///                    "maximum": 1.0,
    ///                    "minimum": 0.0
    ///                  }
    ///                },
    ///                "additionalProperties": false
    ///              },
    ///              "worthiness_floor": {
    ///                "type": "number",
    ///                "maximum": 1.0,
    ///                "minimum": 0.0
    ///              }
    ///            },
    ///            "additionalProperties": false
    ///          }
    ///        },
    ///        "dry_run": {
    ///          "description": "When true, `vox-publisher` forces runtime `syndication.dry_run` during manifest row → UnifiedNewsItem hydration (non-live fan-out regardless of top-level `syndication.dry_run` unless you align both).",
    ///          "type": "boolean"
    ///        },
    ///        "rate_limit_profile": {
    ///          "type": "string"
    ///        },
    ///        "retry_profile": {
    ///          "type": "string"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistribution {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub channel_payloads: ::std::option::Option<ScientiaDistributionChannelPayloads>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub channels: ::std::vec::Vec<ScientiaDistributionChannelsItem>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub distribution_policy: ::std::option::Option<
            ScientiaDistributionDistributionPolicy,
        >,
    }
    impl ::std::default::Default for ScientiaDistribution {
        fn default() -> Self {
            Self {
                channel_payloads: Default::default(),
                channels: Default::default(),
                distribution_policy: Default::default(),
            }
        }
    }
    ///`ScientiaDistributionChannelPayloads`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "crates_io": {
    ///      "description": "Contract placeholder only: `vox-publisher` does not perform crates.io releases yet. If enabled in routing, outcomes are explicit dry-run or not-implemented failures—never silent success.",
    ///      "type": "object",
    ///      "required": [
    ///        "crates_to_update"
    ///      ],
    ///      "properties": {
    ///        "crates_to_update": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string",
    ///            "minLength": 1
    ///          }
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "github": {
    ///      "type": "object",
    ///      "properties": {
    ///        "discussion_category": {
    ///          "type": "string"
    ///        },
    ///        "draft": {
    ///          "type": "boolean"
    ///        },
    ///        "post_type": {
    ///          "type": "string",
    ///          "enum": [
    ///            "Release",
    ///            "Discussion"
    ///          ]
    ///        },
    ///        "release_tag": {
    ///          "type": "string"
    ///        },
    ///        "repo": {
    ///          "type": "string",
    ///          "minLength": 1
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "hacker_news": {
    ///      "type": "object",
    ///      "properties": {
    ///        "mode": {
    ///          "type": "string",
    ///          "enum": [
    ///            "manual_assist"
    ///          ]
    ///        },
    ///        "title_override": {
    ///          "type": "string"
    ///        },
    ///        "url_override": {
    ///          "type": "string"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "open_collective": {
    ///      "type": "object",
    ///      "required": [
    ///        "collective_slug"
    ///      ],
    ///      "properties": {
    ///        "collective_slug": {
    ///          "type": "string",
    ///          "minLength": 1
    ///        },
    ///        "is_private": {
    ///          "type": "boolean"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "reddit": {
    ///      "type": "object",
    ///      "required": [
    ///        "subreddit"
    ///      ],
    ///      "properties": {
    ///        "kind": {
    ///          "type": "string",
    ///          "enum": [
    ///            "link",
    ///            "self_post"
    ///          ]
    ///        },
    ///        "nsfw": {
    ///          "type": "boolean"
    ///        },
    ///        "send_replies": {
    ///          "type": "boolean"
    ///        },
    ///        "spoiler": {
    ///          "type": "boolean"
    ///        },
    ///        "subreddit": {
    ///          "type": "string",
    ///          "minLength": 1
    ///        },
    ///        "text_override": {
    ///          "type": "string"
    ///        },
    ///        "title_override": {
    ///          "type": "string"
    ///        },
    ///        "url_override": {
    ///          "type": "string"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "twitter": {
    ///      "type": "object",
    ///      "properties": {
    ///        "short_text": {
    ///          "type": "string"
    ///        },
    ///        "thread": {
    ///          "type": "boolean"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "youtube": {
    ///      "type": "object",
    ///      "required": [
    ///        "video_asset_ref"
    ///      ],
    ///      "properties": {
    ///        "category_id": {
    ///          "type": "string"
    ///        },
    ///        "description_override": {
    ///          "type": "string"
    ///        },
    ///        "notify_subscribers": {
    ///          "type": "boolean"
    ///        },
    ///        "privacy_status": {
    ///          "type": "string",
    ///          "enum": [
    ///            "private",
    ///            "unlisted",
    ///            "public"
    ///          ]
    ///        },
    ///        "tags": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "title_override": {
    ///          "type": "string"
    ///        },
    ///        "video_asset_ref": {
    ///          "type": "string",
    ///          "minLength": 1
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionChannelPayloads {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub crates_io: ::std::option::Option<ScientiaDistributionChannelPayloadsCratesIo>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub github: ::std::option::Option<ScientiaDistributionChannelPayloadsGithub>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub hacker_news: ::std::option::Option<
            ScientiaDistributionChannelPayloadsHackerNews,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub open_collective: ::std::option::Option<
            ScientiaDistributionChannelPayloadsOpenCollective,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub reddit: ::std::option::Option<ScientiaDistributionChannelPayloadsReddit>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub twitter: ::std::option::Option<ScientiaDistributionChannelPayloadsTwitter>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub youtube: ::std::option::Option<ScientiaDistributionChannelPayloadsYoutube>,
    }
    impl ::std::default::Default for ScientiaDistributionChannelPayloads {
        fn default() -> Self {
            Self {
                crates_io: Default::default(),
                github: Default::default(),
                hacker_news: Default::default(),
                open_collective: Default::default(),
                reddit: Default::default(),
                twitter: Default::default(),
                youtube: Default::default(),
            }
        }
    }
    ///Contract placeholder only: `vox-publisher` does not perform crates.io releases yet. If enabled in routing, outcomes are explicit dry-run or not-implemented failures—never silent success.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Contract placeholder only: `vox-publisher` does not perform crates.io releases yet. If enabled in routing, outcomes are explicit dry-run or not-implemented failures—never silent success.",
    ///  "type": "object",
    ///  "required": [
    ///    "crates_to_update"
    ///  ],
    ///  "properties": {
    ///    "crates_to_update": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "minLength": 1
    ///      }
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionChannelPayloadsCratesIo {
        pub crates_to_update: ::std::vec::Vec<
            ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem,
        >,
    }
    ///`ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem>
    for ::std::string::String {
        fn from(
            value: ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaDistributionChannelPayloadsCratesIoCratesToUpdateItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaDistributionChannelPayloadsGithub`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "discussion_category": {
    ///      "type": "string"
    ///    },
    ///    "draft": {
    ///      "type": "boolean"
    ///    },
    ///    "post_type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "Release",
    ///        "Discussion"
    ///      ]
    ///    },
    ///    "release_tag": {
    ///      "type": "string"
    ///    },
    ///    "repo": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionChannelPayloadsGithub {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub discussion_category: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub draft: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub post_type: ::std::option::Option<
            ScientiaDistributionChannelPayloadsGithubPostType,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub release_tag: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub repo: ::std::option::Option<ScientiaDistributionChannelPayloadsGithubRepo>,
    }
    impl ::std::default::Default for ScientiaDistributionChannelPayloadsGithub {
        fn default() -> Self {
            Self {
                discussion_category: Default::default(),
                draft: Default::default(),
                post_type: Default::default(),
                release_tag: Default::default(),
                repo: Default::default(),
            }
        }
    }
    ///`ScientiaDistributionChannelPayloadsGithubPostType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "Release",
    ///    "Discussion"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaDistributionChannelPayloadsGithubPostType {
        Release,
        Discussion,
    }
    impl ::std::fmt::Display for ScientiaDistributionChannelPayloadsGithubPostType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Release => f.write_str("Release"),
                Self::Discussion => f.write_str("Discussion"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaDistributionChannelPayloadsGithubPostType {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "Release" => Ok(Self::Release),
                "Discussion" => Ok(Self::Discussion),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaDistributionChannelPayloadsGithubPostType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelPayloadsGithubPostType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelPayloadsGithubPostType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaDistributionChannelPayloadsGithubRepo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaDistributionChannelPayloadsGithubRepo(::std::string::String);
    impl ::std::ops::Deref for ScientiaDistributionChannelPayloadsGithubRepo {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaDistributionChannelPayloadsGithubRepo>
    for ::std::string::String {
        fn from(value: ScientiaDistributionChannelPayloadsGithubRepo) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaDistributionChannelPayloadsGithubRepo {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaDistributionChannelPayloadsGithubRepo {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelPayloadsGithubRepo {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelPayloadsGithubRepo {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaDistributionChannelPayloadsGithubRepo {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaDistributionChannelPayloadsHackerNews`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "mode": {
    ///      "type": "string",
    ///      "enum": [
    ///        "manual_assist"
    ///      ]
    ///    },
    ///    "title_override": {
    ///      "type": "string"
    ///    },
    ///    "url_override": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionChannelPayloadsHackerNews {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub mode: ::std::option::Option<ScientiaDistributionChannelPayloadsHackerNewsMode>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title_override: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url_override: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ScientiaDistributionChannelPayloadsHackerNews {
        fn default() -> Self {
            Self {
                mode: Default::default(),
                title_override: Default::default(),
                url_override: Default::default(),
            }
        }
    }
    ///`ScientiaDistributionChannelPayloadsHackerNewsMode`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "manual_assist"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaDistributionChannelPayloadsHackerNewsMode {
        #[serde(rename = "manual_assist")]
        ManualAssist,
    }
    impl ::std::fmt::Display for ScientiaDistributionChannelPayloadsHackerNewsMode {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::ManualAssist => f.write_str("manual_assist"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaDistributionChannelPayloadsHackerNewsMode {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "manual_assist" => Ok(Self::ManualAssist),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaDistributionChannelPayloadsHackerNewsMode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelPayloadsHackerNewsMode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelPayloadsHackerNewsMode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaDistributionChannelPayloadsOpenCollective`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "collective_slug"
    ///  ],
    ///  "properties": {
    ///    "collective_slug": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "is_private": {
    ///      "type": "boolean"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionChannelPayloadsOpenCollective {
        pub collective_slug: ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub is_private: ::std::option::Option<bool>,
    }
    ///`ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<
        ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug,
    > for ::std::string::String {
        fn from(
            value: ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaDistributionChannelPayloadsOpenCollectiveCollectiveSlug {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaDistributionChannelPayloadsReddit`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "subreddit"
    ///  ],
    ///  "properties": {
    ///    "kind": {
    ///      "type": "string",
    ///      "enum": [
    ///        "link",
    ///        "self_post"
    ///      ]
    ///    },
    ///    "nsfw": {
    ///      "type": "boolean"
    ///    },
    ///    "send_replies": {
    ///      "type": "boolean"
    ///    },
    ///    "spoiler": {
    ///      "type": "boolean"
    ///    },
    ///    "subreddit": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "text_override": {
    ///      "type": "string"
    ///    },
    ///    "title_override": {
    ///      "type": "string"
    ///    },
    ///    "url_override": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionChannelPayloadsReddit {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub kind: ::std::option::Option<ScientiaDistributionChannelPayloadsRedditKind>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub nsfw: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub send_replies: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub spoiler: ::std::option::Option<bool>,
        pub subreddit: ScientiaDistributionChannelPayloadsRedditSubreddit,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub text_override: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title_override: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub url_override: ::std::option::Option<::std::string::String>,
    }
    ///`ScientiaDistributionChannelPayloadsRedditKind`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "link",
    ///    "self_post"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaDistributionChannelPayloadsRedditKind {
        #[serde(rename = "link")]
        Link,
        #[serde(rename = "self_post")]
        SelfPost,
    }
    impl ::std::fmt::Display for ScientiaDistributionChannelPayloadsRedditKind {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Link => f.write_str("link"),
                Self::SelfPost => f.write_str("self_post"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaDistributionChannelPayloadsRedditKind {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "link" => Ok(Self::Link),
                "self_post" => Ok(Self::SelfPost),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaDistributionChannelPayloadsRedditKind {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelPayloadsRedditKind {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelPayloadsRedditKind {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaDistributionChannelPayloadsRedditSubreddit`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaDistributionChannelPayloadsRedditSubreddit(::std::string::String);
    impl ::std::ops::Deref for ScientiaDistributionChannelPayloadsRedditSubreddit {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaDistributionChannelPayloadsRedditSubreddit>
    for ::std::string::String {
        fn from(value: ScientiaDistributionChannelPayloadsRedditSubreddit) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaDistributionChannelPayloadsRedditSubreddit {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaDistributionChannelPayloadsRedditSubreddit {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelPayloadsRedditSubreddit {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelPayloadsRedditSubreddit {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaDistributionChannelPayloadsRedditSubreddit {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaDistributionChannelPayloadsTwitter`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "short_text": {
    ///      "type": "string"
    ///    },
    ///    "thread": {
    ///      "type": "boolean"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionChannelPayloadsTwitter {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub short_text: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub thread: ::std::option::Option<bool>,
    }
    impl ::std::default::Default for ScientiaDistributionChannelPayloadsTwitter {
        fn default() -> Self {
            Self {
                short_text: Default::default(),
                thread: Default::default(),
            }
        }
    }
    ///`ScientiaDistributionChannelPayloadsYoutube`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "video_asset_ref"
    ///  ],
    ///  "properties": {
    ///    "category_id": {
    ///      "type": "string"
    ///    },
    ///    "description_override": {
    ///      "type": "string"
    ///    },
    ///    "notify_subscribers": {
    ///      "type": "boolean"
    ///    },
    ///    "privacy_status": {
    ///      "type": "string",
    ///      "enum": [
    ///        "private",
    ///        "unlisted",
    ///        "public"
    ///      ]
    ///    },
    ///    "tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "title_override": {
    ///      "type": "string"
    ///    },
    ///    "video_asset_ref": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionChannelPayloadsYoutube {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub category_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub description_override: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notify_subscribers: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub privacy_status: ::std::option::Option<
            ScientiaDistributionChannelPayloadsYoutubePrivacyStatus,
        >,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title_override: ::std::option::Option<::std::string::String>,
        pub video_asset_ref: ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef,
    }
    ///`ScientiaDistributionChannelPayloadsYoutubePrivacyStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "private",
    ///    "unlisted",
    ///    "public"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaDistributionChannelPayloadsYoutubePrivacyStatus {
        #[serde(rename = "private")]
        Private,
        #[serde(rename = "unlisted")]
        Unlisted,
        #[serde(rename = "public")]
        Public,
    }
    impl ::std::fmt::Display for ScientiaDistributionChannelPayloadsYoutubePrivacyStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Private => f.write_str("private"),
                Self::Unlisted => f.write_str("unlisted"),
                Self::Public => f.write_str("public"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaDistributionChannelPayloadsYoutubePrivacyStatus {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "private" => Ok(Self::Private),
                "unlisted" => Ok(Self::Unlisted),
                "public" => Ok(Self::Public),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaDistributionChannelPayloadsYoutubePrivacyStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelPayloadsYoutubePrivacyStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelPayloadsYoutubePrivacyStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef(
        ::std::string::String,
    );
    impl ::std::ops::Deref for ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef>
    for ::std::string::String {
        fn from(value: ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaDistributionChannelPayloadsYoutubeVideoAssetRef {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaDistributionChannelsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "rss",
    ///    "twitter",
    ///    "github",
    ///    "open_collective",
    ///    "reddit",
    ///    "hacker_news",
    ///    "youtube",
    ///    "crates_io"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaDistributionChannelsItem {
        #[serde(rename = "rss")]
        Rss,
        #[serde(rename = "twitter")]
        Twitter,
        #[serde(rename = "github")]
        Github,
        #[serde(rename = "open_collective")]
        OpenCollective,
        #[serde(rename = "reddit")]
        Reddit,
        #[serde(rename = "hacker_news")]
        HackerNews,
        #[serde(rename = "youtube")]
        Youtube,
        #[serde(rename = "crates_io")]
        CratesIo,
    }
    impl ::std::fmt::Display for ScientiaDistributionChannelsItem {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Rss => f.write_str("rss"),
                Self::Twitter => f.write_str("twitter"),
                Self::Github => f.write_str("github"),
                Self::OpenCollective => f.write_str("open_collective"),
                Self::Reddit => f.write_str("reddit"),
                Self::HackerNews => f.write_str("hacker_news"),
                Self::Youtube => f.write_str("youtube"),
                Self::CratesIo => f.write_str("crates_io"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaDistributionChannelsItem {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "rss" => Ok(Self::Rss),
                "twitter" => Ok(Self::Twitter),
                "github" => Ok(Self::Github),
                "open_collective" => Ok(Self::OpenCollective),
                "reddit" => Ok(Self::Reddit),
                "hacker_news" => Ok(Self::HackerNews),
                "youtube" => Ok(Self::Youtube),
                "crates_io" => Ok(Self::CratesIo),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaDistributionChannelsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaDistributionChannelsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaDistributionChannelsItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaDistributionDistributionPolicy`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "approval_required": {
    ///      "type": "boolean"
    ///    },
    ///    "channel_policy": {
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "type": "object",
    ///        "properties": {
    ///          "enabled": {
    ///            "type": "boolean"
    ///          },
    ///          "template_profile": {
    ///            "type": "string"
    ///          },
    ///          "topic_filters": {
    ///            "type": "object",
    ///            "properties": {
    ///              "exclude_tags": {
    ///                "type": "array",
    ///                "items": {
    ///                  "type": "string"
    ///                }
    ///              },
    ///              "include_tags": {
    ///                "type": "array",
    ///                "items": {
    ///                  "type": "string"
    ///                }
    ///              },
    ///              "min_topic_score": {
    ///                "type": "number",
    ///                "maximum": 1.0,
    ///                "minimum": 0.0
    ///              }
    ///            },
    ///            "additionalProperties": false
    ///          },
    ///          "worthiness_floor": {
    ///            "type": "number",
    ///            "maximum": 1.0,
    ///            "minimum": 0.0
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "dry_run": {
    ///      "description": "When true, `vox-publisher` forces runtime `syndication.dry_run` during manifest row → UnifiedNewsItem hydration (non-live fan-out regardless of top-level `syndication.dry_run` unless you align both).",
    ///      "type": "boolean"
    ///    },
    ///    "rate_limit_profile": {
    ///      "type": "string"
    ///    },
    ///    "retry_profile": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionDistributionPolicy {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub approval_required: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = ":: std :: collections :: HashMap::is_empty")]
        pub channel_policy: ::std::collections::HashMap<
            ::std::string::String,
            ScientiaDistributionDistributionPolicyChannelPolicyValue,
        >,
        ///When true, `vox-publisher` forces runtime `syndication.dry_run` during manifest row → UnifiedNewsItem hydration (non-live fan-out regardless of top-level `syndication.dry_run` unless you align both).
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub dry_run: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub rate_limit_profile: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub retry_profile: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ScientiaDistributionDistributionPolicy {
        fn default() -> Self {
            Self {
                approval_required: Default::default(),
                channel_policy: Default::default(),
                dry_run: Default::default(),
                rate_limit_profile: Default::default(),
                retry_profile: Default::default(),
            }
        }
    }
    ///`ScientiaDistributionDistributionPolicyChannelPolicyValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "enabled": {
    ///      "type": "boolean"
    ///    },
    ///    "template_profile": {
    ///      "type": "string"
    ///    },
    ///    "topic_filters": {
    ///      "type": "object",
    ///      "properties": {
    ///        "exclude_tags": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "include_tags": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "min_topic_score": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "worthiness_floor": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionDistributionPolicyChannelPolicyValue {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub enabled: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub template_profile: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub topic_filters: ::std::option::Option<
            ScientiaDistributionDistributionPolicyChannelPolicyValueTopicFilters,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub worthiness_floor: ::std::option::Option<f64>,
    }
    impl ::std::default::Default
    for ScientiaDistributionDistributionPolicyChannelPolicyValue {
        fn default() -> Self {
            Self {
                enabled: Default::default(),
                template_profile: Default::default(),
                topic_filters: Default::default(),
                worthiness_floor: Default::default(),
            }
        }
    }
    ///`ScientiaDistributionDistributionPolicyChannelPolicyValueTopicFilters`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "exclude_tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "include_tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "min_topic_score": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaDistributionDistributionPolicyChannelPolicyValueTopicFilters {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub exclude_tags: ::std::vec::Vec<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub include_tags: ::std::vec::Vec<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub min_topic_score: ::std::option::Option<f64>,
    }
    impl ::std::default::Default
    for ScientiaDistributionDistributionPolicyChannelPolicyValueTopicFilters {
        fn default() -> Self {
            Self {
                exclude_tags: Default::default(),
                include_tags: Default::default(),
                min_topic_score: Default::default(),
            }
        }
    }
}

// --- contracts/scientia\evidence-pack.v1.schema.json ---
pub mod evidence_pack_v1_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///`RunRef`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "config_digest",
    ///    "run_id"
    ///  ],
    ///  "properties": {
    ///    "config_digest": {
    ///      "type": "string",
    ///      "minLength": 16
    ///    },
    ///    "eval_digest": {
    ///      "type": "string"
    ///    },
    ///    "gate_digest": {
    ///      "type": "string"
    ///    },
    ///    "run_id": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "telemetry_digest": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct RunRef {
        pub config_digest: RunRefConfigDigest,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub eval_digest: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub gate_digest: ::std::option::Option<::std::string::String>,
        pub run_id: RunRefRunId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub telemetry_digest: ::std::option::Option<::std::string::String>,
    }
    ///`RunRefConfigDigest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 16
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct RunRefConfigDigest(::std::string::String);
    impl ::std::ops::Deref for RunRefConfigDigest {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<RunRefConfigDigest> for ::std::string::String {
        fn from(value: RunRefConfigDigest) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for RunRefConfigDigest {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 16usize {
                return Err("shorter than 16 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for RunRefConfigDigest {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for RunRefConfigDigest {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for RunRefConfigDigest {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for RunRefConfigDigest {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`RunRefRunId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct RunRefRunId(::std::string::String);
    impl ::std::ops::Deref for RunRefRunId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<RunRefRunId> for ::std::string::String {
        fn from(value: RunRefRunId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for RunRefRunId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for RunRefRunId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for RunRefRunId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for RunRefRunId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for RunRefRunId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaEvidencePackV1`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/evidence-pack.v1.schema.json",
    ///  "title": "SCIENTIA EvidencePack v1",
    ///  "type": "object",
    ///  "required": [
    ///    "baseline",
    ///    "candidate",
    ///    "manifest_digest",
    ///    "publication_id",
    ///    "replay_instructions",
    ///    "version"
    ///  ],
    ///  "properties": {
    ///    "baseline": {
    ///      "$ref": "#/$defs/runRef"
    ///    },
    ///    "candidate": {
    ///      "$ref": "#/$defs/runRef"
    ///    },
    ///    "manifest_digest": {
    ///      "type": "string",
    ///      "minLength": 16
    ///    },
    ///    "pair_integrity_passed": {
    ///      "type": "boolean"
    ///    },
    ///    "publication_id": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "replay_instructions": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "version": {
    ///      "type": "string",
    ///      "const": "v1"
    ///    }
    ///  },
    ///  "additionalProperties": false,
    ///  "x-vox-version": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaEvidencePackV1 {
        pub baseline: RunRef,
        pub candidate: RunRef,
        pub manifest_digest: ScientiaEvidencePackV1ManifestDigest,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pair_integrity_passed: ::std::option::Option<bool>,
        pub publication_id: ScientiaEvidencePackV1PublicationId,
        pub replay_instructions: ScientiaEvidencePackV1ReplayInstructions,
        pub version: ::std::string::String,
    }
    ///`ScientiaEvidencePackV1ManifestDigest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 16
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaEvidencePackV1ManifestDigest(::std::string::String);
    impl ::std::ops::Deref for ScientiaEvidencePackV1ManifestDigest {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaEvidencePackV1ManifestDigest>
    for ::std::string::String {
        fn from(value: ScientiaEvidencePackV1ManifestDigest) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaEvidencePackV1ManifestDigest {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 16usize {
                return Err("shorter than 16 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaEvidencePackV1ManifestDigest {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaEvidencePackV1ManifestDigest {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaEvidencePackV1ManifestDigest {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaEvidencePackV1ManifestDigest {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaEvidencePackV1PublicationId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaEvidencePackV1PublicationId(::std::string::String);
    impl ::std::ops::Deref for ScientiaEvidencePackV1PublicationId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaEvidencePackV1PublicationId>
    for ::std::string::String {
        fn from(value: ScientiaEvidencePackV1PublicationId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaEvidencePackV1PublicationId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaEvidencePackV1PublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaEvidencePackV1PublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaEvidencePackV1PublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaEvidencePackV1PublicationId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaEvidencePackV1ReplayInstructions`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaEvidencePackV1ReplayInstructions(::std::string::String);
    impl ::std::ops::Deref for ScientiaEvidencePackV1ReplayInstructions {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaEvidencePackV1ReplayInstructions>
    for ::std::string::String {
        fn from(value: ScientiaEvidencePackV1ReplayInstructions) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaEvidencePackV1ReplayInstructions {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaEvidencePackV1ReplayInstructions {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaEvidencePackV1ReplayInstructions {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaEvidencePackV1ReplayInstructions {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaEvidencePackV1ReplayInstructions {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
}

// --- contracts/scientia\finding-candidate.v1.schema.json ---
pub mod finding_candidate_v1_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///`DiscoverySignalShape`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "code",
    ///    "family",
    ///    "provenance",
    ///    "strength",
    ///    "summary"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "family": {
    ///      "type": "string",
    ///      "enum": [
    ///        "unspecified",
    ///        "eval_gate",
    ///        "benchmark_pair",
    ///        "documentation",
    ///        "telemetry_aggregate",
    ///        "operator_attestation",
    ///        "mens_scorecard",
    ///        "trust_rollup",
    ///        "reproducibility_artifact",
    ///        "linked_corpus",
    ///        "finding_candidate_signal"
    ///      ]
    ///    },
    ///    "provenance": {
    ///      "type": "object",
    ///      "properties": {
    ///        "digest": {
    ///          "type": "string"
    ///        },
    ///        "metric_type": {
    ///          "type": "string"
    ///        },
    ///        "origin": {
    ///          "type": "string"
    ///        },
    ///        "recorded_at_ms": {
    ///          "type": "integer"
    ///        },
    ///        "repo_path": {
    ///          "type": "string"
    ///        },
    ///        "run_id": {
    ///          "type": "string"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "source_ref": {
    ///      "type": "string"
    ///    },
    ///    "strength": {
    ///      "type": "string",
    ///      "enum": [
    ///        "supporting",
    ///        "strong",
    ///        "informational"
    ///      ]
    ///    },
    ///    "summary": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct DiscoverySignalShape {
        pub code: DiscoverySignalShapeCode,
        pub family: DiscoverySignalShapeFamily,
        pub provenance: DiscoverySignalShapeProvenance,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source_ref: ::std::option::Option<::std::string::String>,
        pub strength: DiscoverySignalShapeStrength,
        pub summary: ::std::string::String,
    }
    ///`DiscoverySignalShapeCode`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct DiscoverySignalShapeCode(::std::string::String);
    impl ::std::ops::Deref for DiscoverySignalShapeCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<DiscoverySignalShapeCode> for ::std::string::String {
        fn from(value: DiscoverySignalShapeCode) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for DiscoverySignalShapeCode {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for DiscoverySignalShapeCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for DiscoverySignalShapeCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for DiscoverySignalShapeCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for DiscoverySignalShapeCode {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`DiscoverySignalShapeFamily`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "unspecified",
    ///    "eval_gate",
    ///    "benchmark_pair",
    ///    "documentation",
    ///    "telemetry_aggregate",
    ///    "operator_attestation",
    ///    "mens_scorecard",
    ///    "trust_rollup",
    ///    "reproducibility_artifact",
    ///    "linked_corpus",
    ///    "finding_candidate_signal"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum DiscoverySignalShapeFamily {
        #[serde(rename = "unspecified")]
        Unspecified,
        #[serde(rename = "eval_gate")]
        EvalGate,
        #[serde(rename = "benchmark_pair")]
        BenchmarkPair,
        #[serde(rename = "documentation")]
        Documentation,
        #[serde(rename = "telemetry_aggregate")]
        TelemetryAggregate,
        #[serde(rename = "operator_attestation")]
        OperatorAttestation,
        #[serde(rename = "mens_scorecard")]
        MensScorecard,
        #[serde(rename = "trust_rollup")]
        TrustRollup,
        #[serde(rename = "reproducibility_artifact")]
        ReproducibilityArtifact,
        #[serde(rename = "linked_corpus")]
        LinkedCorpus,
        #[serde(rename = "finding_candidate_signal")]
        FindingCandidateSignal,
    }
    impl ::std::fmt::Display for DiscoverySignalShapeFamily {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Unspecified => f.write_str("unspecified"),
                Self::EvalGate => f.write_str("eval_gate"),
                Self::BenchmarkPair => f.write_str("benchmark_pair"),
                Self::Documentation => f.write_str("documentation"),
                Self::TelemetryAggregate => f.write_str("telemetry_aggregate"),
                Self::OperatorAttestation => f.write_str("operator_attestation"),
                Self::MensScorecard => f.write_str("mens_scorecard"),
                Self::TrustRollup => f.write_str("trust_rollup"),
                Self::ReproducibilityArtifact => f.write_str("reproducibility_artifact"),
                Self::LinkedCorpus => f.write_str("linked_corpus"),
                Self::FindingCandidateSignal => f.write_str("finding_candidate_signal"),
            }
        }
    }
    impl ::std::str::FromStr for DiscoverySignalShapeFamily {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "unspecified" => Ok(Self::Unspecified),
                "eval_gate" => Ok(Self::EvalGate),
                "benchmark_pair" => Ok(Self::BenchmarkPair),
                "documentation" => Ok(Self::Documentation),
                "telemetry_aggregate" => Ok(Self::TelemetryAggregate),
                "operator_attestation" => Ok(Self::OperatorAttestation),
                "mens_scorecard" => Ok(Self::MensScorecard),
                "trust_rollup" => Ok(Self::TrustRollup),
                "reproducibility_artifact" => Ok(Self::ReproducibilityArtifact),
                "linked_corpus" => Ok(Self::LinkedCorpus),
                "finding_candidate_signal" => Ok(Self::FindingCandidateSignal),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for DiscoverySignalShapeFamily {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for DiscoverySignalShapeFamily {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for DiscoverySignalShapeFamily {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`DiscoverySignalShapeProvenance`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "digest": {
    ///      "type": "string"
    ///    },
    ///    "metric_type": {
    ///      "type": "string"
    ///    },
    ///    "origin": {
    ///      "type": "string"
    ///    },
    ///    "recorded_at_ms": {
    ///      "type": "integer"
    ///    },
    ///    "repo_path": {
    ///      "type": "string"
    ///    },
    ///    "run_id": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct DiscoverySignalShapeProvenance {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub digest: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub metric_type: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub origin: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub recorded_at_ms: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub repo_path: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub run_id: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for DiscoverySignalShapeProvenance {
        fn default() -> Self {
            Self {
                digest: Default::default(),
                metric_type: Default::default(),
                origin: Default::default(),
                recorded_at_ms: Default::default(),
                repo_path: Default::default(),
                run_id: Default::default(),
            }
        }
    }
    ///`DiscoverySignalShapeStrength`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "supporting",
    ///    "strong",
    ///    "informational"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum DiscoverySignalShapeStrength {
        #[serde(rename = "supporting")]
        Supporting,
        #[serde(rename = "strong")]
        Strong,
        #[serde(rename = "informational")]
        Informational,
    }
    impl ::std::fmt::Display for DiscoverySignalShapeStrength {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Supporting => f.write_str("supporting"),
                Self::Strong => f.write_str("strong"),
                Self::Informational => f.write_str("informational"),
            }
        }
    }
    impl ::std::str::FromStr for DiscoverySignalShapeStrength {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "supporting" => Ok(Self::Supporting),
                "strong" => Ok(Self::Strong),
                "informational" => Ok(Self::Informational),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for DiscoverySignalShapeStrength {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for DiscoverySignalShapeStrength {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for DiscoverySignalShapeStrength {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///Canonical candidate from internal signals through novelty evidence to worthiness decision traces. internal_signals align with discovery-signal.schema.json (validated separately or embedded here).
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/finding-candidate.v1.schema.json",
    ///  "title": "SCIENTIA finding candidate ledger record v1",
    ///  "description": "Canonical candidate from internal signals through novelty evidence to worthiness decision traces. internal_signals align with discovery-signal.schema.json (validated separately or embedded here).",
    ///  "type": "object",
    ///  "required": [
    ///    "candidate_class",
    ///    "candidate_id",
    ///    "created_at_ms",
    ///    "internal_signals",
    ///    "schema_version"
    ///  ],
    ///  "properties": {
    ///    "candidate_class": {
    ///      "type": "string",
    ///      "enum": [
    ///        "algorithmic_improvement",
    ///        "reproducibility_infra",
    ///        "policy_governance",
    ///        "telemetry_trust",
    ///        "other"
    ///      ]
    ///    },
    ///    "candidate_id": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "confidence": {
    ///      "type": "object",
    ///      "properties": {
    ///        "contradiction_risk": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "reproducibility_support": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "signal_strength": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "created_at_ms": {
    ///      "type": "integer"
    ///    },
    ///    "internal_signals": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/$defs/discovery_signal_shape"
    ///      }
    ///    },
    ///    "novelty_evidence_bundle_id": {
    ///      "type": "string"
    ///    },
    ///    "publication_id": {
    ///      "type": "string"
    ///    },
    ///    "schema_version": {
    ///      "type": "integer",
    ///      "const": 1
    ///    },
    ///    "title_hint": {
    ///      "type": "string"
    ///    },
    ///    "updated_at_ms": {
    ///      "type": "integer"
    ///    },
    ///    "worthiness_decision_ref": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false,
    ///  "x-vox-version": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaFindingCandidateLedgerRecordV1 {
        pub candidate_class: ScientiaFindingCandidateLedgerRecordV1CandidateClass,
        pub candidate_id: ScientiaFindingCandidateLedgerRecordV1CandidateId,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub confidence: ::std::option::Option<
            ScientiaFindingCandidateLedgerRecordV1Confidence,
        >,
        pub created_at_ms: i64,
        pub internal_signals: ::std::vec::Vec<DiscoverySignalShape>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub novelty_evidence_bundle_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub publication_id: ::std::option::Option<::std::string::String>,
        pub schema_version: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title_hint: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub updated_at_ms: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub worthiness_decision_ref: ::std::option::Option<::std::string::String>,
    }
    ///`ScientiaFindingCandidateLedgerRecordV1CandidateClass`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "algorithmic_improvement",
    ///    "reproducibility_infra",
    ///    "policy_governance",
    ///    "telemetry_trust",
    ///    "other"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaFindingCandidateLedgerRecordV1CandidateClass {
        #[serde(rename = "algorithmic_improvement")]
        AlgorithmicImprovement,
        #[serde(rename = "reproducibility_infra")]
        ReproducibilityInfra,
        #[serde(rename = "policy_governance")]
        PolicyGovernance,
        #[serde(rename = "telemetry_trust")]
        TelemetryTrust,
        #[serde(rename = "other")]
        Other,
    }
    impl ::std::fmt::Display for ScientiaFindingCandidateLedgerRecordV1CandidateClass {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::AlgorithmicImprovement => f.write_str("algorithmic_improvement"),
                Self::ReproducibilityInfra => f.write_str("reproducibility_infra"),
                Self::PolicyGovernance => f.write_str("policy_governance"),
                Self::TelemetryTrust => f.write_str("telemetry_trust"),
                Self::Other => f.write_str("other"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaFindingCandidateLedgerRecordV1CandidateClass {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "algorithmic_improvement" => Ok(Self::AlgorithmicImprovement),
                "reproducibility_infra" => Ok(Self::ReproducibilityInfra),
                "policy_governance" => Ok(Self::PolicyGovernance),
                "telemetry_trust" => Ok(Self::TelemetryTrust),
                "other" => Ok(Self::Other),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaFindingCandidateLedgerRecordV1CandidateClass {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaFindingCandidateLedgerRecordV1CandidateClass {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaFindingCandidateLedgerRecordV1CandidateClass {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaFindingCandidateLedgerRecordV1CandidateId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaFindingCandidateLedgerRecordV1CandidateId(::std::string::String);
    impl ::std::ops::Deref for ScientiaFindingCandidateLedgerRecordV1CandidateId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaFindingCandidateLedgerRecordV1CandidateId>
    for ::std::string::String {
        fn from(value: ScientiaFindingCandidateLedgerRecordV1CandidateId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaFindingCandidateLedgerRecordV1CandidateId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaFindingCandidateLedgerRecordV1CandidateId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaFindingCandidateLedgerRecordV1CandidateId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaFindingCandidateLedgerRecordV1CandidateId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaFindingCandidateLedgerRecordV1CandidateId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaFindingCandidateLedgerRecordV1Confidence`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "contradiction_risk": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "reproducibility_support": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "signal_strength": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaFindingCandidateLedgerRecordV1Confidence {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub contradiction_risk: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub reproducibility_support: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub signal_strength: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for ScientiaFindingCandidateLedgerRecordV1Confidence {
        fn default() -> Self {
            Self {
                contradiction_risk: Default::default(),
                reproducibility_support: Default::default(),
                signal_strength: Default::default(),
            }
        }
    }
}

// --- contracts/scientia\machine-suggestion-block.schema.json ---
pub mod machine_suggestion_block_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///LLM or heuristic outputs must carry these labels; never treated as ground truth.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/machine-suggestion-block.schema.json",
    ///  "title": "SCIENTIA machine suggestion envelope",
    ///  "description": "LLM or heuristic outputs must carry these labels; never treated as ground truth.",
    ///  "type": "object",
    ///  "required": [
    ///    "machine_suggested",
    ///    "requires_human_review"
    ///  ],
    ///  "properties": {
    ///    "machine_suggested": {
    ///      "type": "boolean",
    ///      "const": true
    ///    },
    ///    "requires_human_review": {
    ///      "type": "boolean",
    ///      "const": true
    ///    },
    ///    "schema_version": {
    ///      "type": "integer"
    ///    },
    ///    "source_grounded": {
    ///      "type": "boolean"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ScientiaMachineSuggestionEnvelope {
        pub machine_suggested: bool,
        pub requires_human_review: bool,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub schema_version: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source_grounded: ::std::option::Option<bool>,
    }
}

// --- contracts/scientia\manifest-completion.schema.json ---
pub mod manifest_completion_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///`ScientiaManifestCompletionReport`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/manifest-completion.schema.json",
    ///  "title": "SCIENTIA manifest completion report",
    ///  "type": "object",
    ///  "required": [
    ///    "completeness_0_100",
    ///    "field_provenance",
    ///    "human_only_pending",
    ///    "inferred_ok",
    ///    "required_missing"
    ///  ],
    ///  "properties": {
    ///    "completeness_0_100": {
    ///      "type": "integer",
    ///      "maximum": 100.0,
    ///      "minimum": 0.0
    ///    },
    ///    "field_provenance": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "field",
    ///          "origin"
    ///        ],
    ///        "properties": {
    ///          "field": {
    ///            "type": "string"
    ///          },
    ///          "notes": {
    ///            "type": "string"
    ///          },
    ///          "origin": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "human_only_pending": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "inferred_ok": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "required_missing": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaManifestCompletionReport {
        pub completeness_0_100: i64,
        pub field_provenance: ::std::vec::Vec<
            ScientiaManifestCompletionReportFieldProvenanceItem,
        >,
        pub human_only_pending: ::std::vec::Vec<::std::string::String>,
        pub inferred_ok: ::std::vec::Vec<::std::string::String>,
        pub required_missing: ::std::vec::Vec<::std::string::String>,
    }
    ///`ScientiaManifestCompletionReportFieldProvenanceItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "field",
    ///    "origin"
    ///  ],
    ///  "properties": {
    ///    "field": {
    ///      "type": "string"
    ///    },
    ///    "notes": {
    ///      "type": "string"
    ///    },
    ///    "origin": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaManifestCompletionReportFieldProvenanceItem {
        pub field: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notes: ::std::option::Option<::std::string::String>,
        pub origin: ::std::string::String,
    }
}

// --- contracts/scientia\novelty-evidence-bundle.v1.schema.json ---
pub mod novelty_evidence_bundle_v1_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///`NormalizedHit`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "source",
    ///    "title",
    ///    "work_uri"
    ///  ],
    ///  "properties": {
    ///    "cited_by_count": {
    ///      "description": "Optional citation count from upstream API (e.g. OpenAlex, Semantic Scholar).",
    ///      "type": "integer",
    ///      "minimum": 0.0
    ///    },
    ///    "lexical_score": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "overlap_note": {
    ///      "type": "string"
    ///    },
    ///    "semantic_score": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "source": {
    ///      "type": "string",
    ///      "enum": [
    ///        "openalex",
    ///        "crossref",
    ///        "semantic_scholar",
    ///        "manual",
    ///        "other"
    ///      ]
    ///    },
    ///    "title": {
    ///      "type": "string"
    ///    },
    ///    "work_uri": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "year": {
    ///      "type": "integer"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct NormalizedHit {
        ///Optional citation count from upstream API (e.g. OpenAlex, Semantic Scholar).
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub cited_by_count: ::std::option::Option<u64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub lexical_score: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub overlap_note: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub semantic_score: ::std::option::Option<f64>,
        pub source: NormalizedHitSource,
        pub title: ::std::string::String,
        pub work_uri: NormalizedHitWorkUri,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub year: ::std::option::Option<i64>,
    }
    ///`NormalizedHitSource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "openalex",
    ///    "crossref",
    ///    "semantic_scholar",
    ///    "manual",
    ///    "other"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum NormalizedHitSource {
        #[serde(rename = "openalex")]
        Openalex,
        #[serde(rename = "crossref")]
        Crossref,
        #[serde(rename = "semantic_scholar")]
        SemanticScholar,
        #[serde(rename = "manual")]
        Manual,
        #[serde(rename = "other")]
        Other,
    }
    impl ::std::fmt::Display for NormalizedHitSource {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Openalex => f.write_str("openalex"),
                Self::Crossref => f.write_str("crossref"),
                Self::SemanticScholar => f.write_str("semantic_scholar"),
                Self::Manual => f.write_str("manual"),
                Self::Other => f.write_str("other"),
            }
        }
    }
    impl ::std::str::FromStr for NormalizedHitSource {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "openalex" => Ok(Self::Openalex),
                "crossref" => Ok(Self::Crossref),
                "semantic_scholar" => Ok(Self::SemanticScholar),
                "manual" => Ok(Self::Manual),
                "other" => Ok(Self::Other),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for NormalizedHitSource {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for NormalizedHitSource {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for NormalizedHitSource {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`NormalizedHitWorkUri`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct NormalizedHitWorkUri(::std::string::String);
    impl ::std::ops::Deref for NormalizedHitWorkUri {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<NormalizedHitWorkUri> for ::std::string::String {
        fn from(value: NormalizedHitWorkUri) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for NormalizedHitWorkUri {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for NormalizedHitWorkUri {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for NormalizedHitWorkUri {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for NormalizedHitWorkUri {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for NormalizedHitWorkUri {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///Normalized prior-art / overlap snapshot from federated sources (OpenAlex, Crossref, Semantic Scholar, …) with deterministic digests for audit.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/novelty-evidence-bundle.v1.schema.json",
    ///  "title": "SCIENTIA novelty evidence bundle v1",
    ///  "description": "Normalized prior-art / overlap snapshot from federated sources (OpenAlex, Crossref, Semantic Scholar, …) with deterministic digests for audit.",
    ///  "type": "object",
    ///  "required": [
    ///    "bundle_id",
    ///    "candidate_id",
    ///    "computed_at_ms",
    ///    "normalized_hits",
    ///    "query_digest_sha256",
    ///    "schema_version",
    ///    "sources"
    ///  ],
    ///  "properties": {
    ///    "bundle_id": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "candidate_id": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "computed_at_ms": {
    ///      "type": "integer"
    ///    },
    ///    "normalized_hits": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/$defs/normalized_hit"
    ///      }
    ///    },
    ///    "overlap_summary": {
    ///      "type": "object",
    ///      "properties": {
    ///        "max_lexical_score": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "max_semantic_score": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "recency_bucket": {
    ///          "type": "string",
    ///          "enum": [
    ///            "unknown",
    ///            "stale",
    ///            "recent",
    ///            "very_recent"
    ///          ]
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "query_digest_sha256": {
    ///      "type": "string",
    ///      "pattern": "^[a-f0-9]{64}$"
    ///    },
    ///    "query_traces": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "request_fingerprint_sha256",
    ///          "source"
    ///        ],
    ///        "properties": {
    ///          "cached": {
    ///            "type": "boolean"
    ///          },
    ///          "http_status": {
    ///            "type": "integer"
    ///          },
    ///          "request_fingerprint_sha256": {
    ///            "type": "string",
    ///            "pattern": "^[a-f0-9]{64}$"
    ///          },
    ///          "source": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "schema_version": {
    ///      "type": "integer",
    ///      "const": 1
    ///    },
    ///    "sources": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "enum": [
    ///          "openalex",
    ///          "crossref",
    ///          "semantic_scholar",
    ///          "manual",
    ///          "other"
    ///        ]
    ///      }
    ///    }
    ///  },
    ///  "additionalProperties": false,
    ///  "x-vox-version": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaNoveltyEvidenceBundleV1 {
        pub bundle_id: ScientiaNoveltyEvidenceBundleV1BundleId,
        pub candidate_id: ScientiaNoveltyEvidenceBundleV1CandidateId,
        pub computed_at_ms: i64,
        pub normalized_hits: ::std::vec::Vec<NormalizedHit>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub overlap_summary: ::std::option::Option<
            ScientiaNoveltyEvidenceBundleV1OverlapSummary,
        >,
        pub query_digest_sha256: ScientiaNoveltyEvidenceBundleV1QueryDigestSha256,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub query_traces: ::std::vec::Vec<ScientiaNoveltyEvidenceBundleV1QueryTracesItem>,
        pub schema_version: i64,
        pub sources: ::std::vec::Vec<ScientiaNoveltyEvidenceBundleV1SourcesItem>,
    }
    ///`ScientiaNoveltyEvidenceBundleV1BundleId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaNoveltyEvidenceBundleV1BundleId(::std::string::String);
    impl ::std::ops::Deref for ScientiaNoveltyEvidenceBundleV1BundleId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaNoveltyEvidenceBundleV1BundleId>
    for ::std::string::String {
        fn from(value: ScientiaNoveltyEvidenceBundleV1BundleId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaNoveltyEvidenceBundleV1BundleId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaNoveltyEvidenceBundleV1BundleId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1BundleId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1BundleId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaNoveltyEvidenceBundleV1BundleId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaNoveltyEvidenceBundleV1CandidateId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaNoveltyEvidenceBundleV1CandidateId(::std::string::String);
    impl ::std::ops::Deref for ScientiaNoveltyEvidenceBundleV1CandidateId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaNoveltyEvidenceBundleV1CandidateId>
    for ::std::string::String {
        fn from(value: ScientiaNoveltyEvidenceBundleV1CandidateId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaNoveltyEvidenceBundleV1CandidateId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaNoveltyEvidenceBundleV1CandidateId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1CandidateId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1CandidateId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaNoveltyEvidenceBundleV1CandidateId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaNoveltyEvidenceBundleV1OverlapSummary`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "max_lexical_score": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "max_semantic_score": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "recency_bucket": {
    ///      "type": "string",
    ///      "enum": [
    ///        "unknown",
    ///        "stale",
    ///        "recent",
    ///        "very_recent"
    ///      ]
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaNoveltyEvidenceBundleV1OverlapSummary {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub max_lexical_score: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub max_semantic_score: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub recency_bucket: ::std::option::Option<
            ScientiaNoveltyEvidenceBundleV1OverlapSummaryRecencyBucket,
        >,
    }
    impl ::std::default::Default for ScientiaNoveltyEvidenceBundleV1OverlapSummary {
        fn default() -> Self {
            Self {
                max_lexical_score: Default::default(),
                max_semantic_score: Default::default(),
                recency_bucket: Default::default(),
            }
        }
    }
    ///`ScientiaNoveltyEvidenceBundleV1OverlapSummaryRecencyBucket`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "unknown",
    ///    "stale",
    ///    "recent",
    ///    "very_recent"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaNoveltyEvidenceBundleV1OverlapSummaryRecencyBucket {
        #[serde(rename = "unknown")]
        Unknown,
        #[serde(rename = "stale")]
        Stale,
        #[serde(rename = "recent")]
        Recent,
        #[serde(rename = "very_recent")]
        VeryRecent,
    }
    impl ::std::fmt::Display for ScientiaNoveltyEvidenceBundleV1OverlapSummaryRecencyBucket {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Unknown => f.write_str("unknown"),
                Self::Stale => f.write_str("stale"),
                Self::Recent => f.write_str("recent"),
                Self::VeryRecent => f.write_str("very_recent"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaNoveltyEvidenceBundleV1OverlapSummaryRecencyBucket {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "unknown" => Ok(Self::Unknown),
                "stale" => Ok(Self::Stale),
                "recent" => Ok(Self::Recent),
                "very_recent" => Ok(Self::VeryRecent),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaNoveltyEvidenceBundleV1OverlapSummaryRecencyBucket {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1OverlapSummaryRecencyBucket {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1OverlapSummaryRecencyBucket {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`ScientiaNoveltyEvidenceBundleV1QueryDigestSha256`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "pattern": "^[a-f0-9]{64}$"
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaNoveltyEvidenceBundleV1QueryDigestSha256(::std::string::String);
    impl ::std::ops::Deref for ScientiaNoveltyEvidenceBundleV1QueryDigestSha256 {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaNoveltyEvidenceBundleV1QueryDigestSha256>
    for ::std::string::String {
        fn from(value: ScientiaNoveltyEvidenceBundleV1QueryDigestSha256) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaNoveltyEvidenceBundleV1QueryDigestSha256 {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> = ::std::sync::LazyLock::new(||
            { ::regress::Regex::new("^[a-f0-9]{64}$").unwrap() });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[a-f0-9]{64}$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaNoveltyEvidenceBundleV1QueryDigestSha256 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1QueryDigestSha256 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1QueryDigestSha256 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaNoveltyEvidenceBundleV1QueryDigestSha256 {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaNoveltyEvidenceBundleV1QueryTracesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "request_fingerprint_sha256",
    ///    "source"
    ///  ],
    ///  "properties": {
    ///    "cached": {
    ///      "type": "boolean"
    ///    },
    ///    "http_status": {
    ///      "type": "integer"
    ///    },
    ///    "request_fingerprint_sha256": {
    ///      "type": "string",
    ///      "pattern": "^[a-f0-9]{64}$"
    ///    },
    ///    "source": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaNoveltyEvidenceBundleV1QueryTracesItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub cached: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub http_status: ::std::option::Option<i64>,
        pub request_fingerprint_sha256: ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256,
        pub source: ::std::string::String,
    }
    ///`ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "pattern": "^[a-f0-9]{64}$"
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256 {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<
        ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256,
    > for ::std::string::String {
        fn from(
            value: ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256 {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> = ::std::sync::LazyLock::new(||
            { ::regress::Regex::new("^[a-f0-9]{64}$").unwrap() });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[a-f0-9]{64}$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256 {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaNoveltyEvidenceBundleV1QueryTracesItemRequestFingerprintSha256 {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaNoveltyEvidenceBundleV1SourcesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "openalex",
    ///    "crossref",
    ///    "semantic_scholar",
    ///    "manual",
    ///    "other"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaNoveltyEvidenceBundleV1SourcesItem {
        #[serde(rename = "openalex")]
        Openalex,
        #[serde(rename = "crossref")]
        Crossref,
        #[serde(rename = "semantic_scholar")]
        SemanticScholar,
        #[serde(rename = "manual")]
        Manual,
        #[serde(rename = "other")]
        Other,
    }
    impl ::std::fmt::Display for ScientiaNoveltyEvidenceBundleV1SourcesItem {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Openalex => f.write_str("openalex"),
                Self::Crossref => f.write_str("crossref"),
                Self::SemanticScholar => f.write_str("semantic_scholar"),
                Self::Manual => f.write_str("manual"),
                Self::Other => f.write_str("other"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaNoveltyEvidenceBundleV1SourcesItem {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "openalex" => Ok(Self::Openalex),
                "crossref" => Ok(Self::Crossref),
                "semantic_scholar" => Ok(Self::SemanticScholar),
                "manual" => Ok(Self::Manual),
                "other" => Ok(Self::Other),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaNoveltyEvidenceBundleV1SourcesItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1SourcesItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaNoveltyEvidenceBundleV1SourcesItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
}

// --- contracts/scientia\operator-status-surface.v1.schema.json ---
pub mod operator_status_surface_v1_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///Shared CLI/MCP read-model contract for publication status views.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/operator-status-surface.v1.schema.json",
    ///  "title": "SCIENTIA operator status surface v1",
    ///  "description": "Shared CLI/MCP read-model contract for publication status views.",
    ///  "type": "object",
    ///  "required": [
    ///    "next_actions",
    ///    "profile",
    ///    "publication_id",
    ///    "route_readiness",
    ///    "snapshot_summary"
    ///  ],
    ///  "properties": {
    ///    "next_actions": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "action",
    ///          "priority"
    ///        ],
    ///        "properties": {
    ///          "action": {
    ///            "type": "string"
    ///          },
    ///          "priority": {
    ///            "type": "integer",
    ///            "maximum": 999.0,
    ///            "minimum": 1.0
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "profile": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "publication_id": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "route_readiness": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "ready",
    ///          "route"
    ///        ],
    ///        "properties": {
    ///          "missing_required": {
    ///            "type": "array",
    ///            "items": {
    ///              "type": "string"
    ///            }
    ///          },
    ///          "ready": {
    ///            "type": "boolean"
    ///          },
    ///          "route": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "snapshot_summary": {
    ///      "type": "object",
    ///      "required": [
    ///        "hard_gate_failures",
    ///        "soft_gate_failures"
    ///      ],
    ///      "properties": {
    ///        "diagnostic_count": {
    ///          "type": "integer",
    ///          "minimum": 0.0
    ///        },
    ///        "hard_gate_failures": {
    ///          "type": "integer",
    ///          "minimum": 0.0
    ///        },
    ///        "soft_gate_failures": {
    ///          "type": "integer",
    ///          "minimum": 0.0
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    }
    ///  },
    ///  "additionalProperties": false,
    ///  "x-vox-version": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaOperatorStatusSurfaceV1 {
        pub next_actions: ::std::vec::Vec<ScientiaOperatorStatusSurfaceV1NextActionsItem>,
        pub profile: ScientiaOperatorStatusSurfaceV1Profile,
        pub publication_id: ScientiaOperatorStatusSurfaceV1PublicationId,
        pub route_readiness: ::std::vec::Vec<
            ScientiaOperatorStatusSurfaceV1RouteReadinessItem,
        >,
        pub snapshot_summary: ScientiaOperatorStatusSurfaceV1SnapshotSummary,
    }
    ///`ScientiaOperatorStatusSurfaceV1NextActionsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "action",
    ///    "priority"
    ///  ],
    ///  "properties": {
    ///    "action": {
    ///      "type": "string"
    ///    },
    ///    "priority": {
    ///      "type": "integer",
    ///      "maximum": 999.0,
    ///      "minimum": 1.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaOperatorStatusSurfaceV1NextActionsItem {
        pub action: ::std::string::String,
        pub priority: ::std::num::NonZeroU64,
    }
    ///`ScientiaOperatorStatusSurfaceV1Profile`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaOperatorStatusSurfaceV1Profile(::std::string::String);
    impl ::std::ops::Deref for ScientiaOperatorStatusSurfaceV1Profile {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaOperatorStatusSurfaceV1Profile>
    for ::std::string::String {
        fn from(value: ScientiaOperatorStatusSurfaceV1Profile) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaOperatorStatusSurfaceV1Profile {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaOperatorStatusSurfaceV1Profile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaOperatorStatusSurfaceV1Profile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaOperatorStatusSurfaceV1Profile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaOperatorStatusSurfaceV1Profile {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaOperatorStatusSurfaceV1PublicationId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaOperatorStatusSurfaceV1PublicationId(::std::string::String);
    impl ::std::ops::Deref for ScientiaOperatorStatusSurfaceV1PublicationId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaOperatorStatusSurfaceV1PublicationId>
    for ::std::string::String {
        fn from(value: ScientiaOperatorStatusSurfaceV1PublicationId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaOperatorStatusSurfaceV1PublicationId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaOperatorStatusSurfaceV1PublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaOperatorStatusSurfaceV1PublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaOperatorStatusSurfaceV1PublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaOperatorStatusSurfaceV1PublicationId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaOperatorStatusSurfaceV1RouteReadinessItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "ready",
    ///    "route"
    ///  ],
    ///  "properties": {
    ///    "missing_required": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "ready": {
    ///      "type": "boolean"
    ///    },
    ///    "route": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaOperatorStatusSurfaceV1RouteReadinessItem {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub missing_required: ::std::vec::Vec<::std::string::String>,
        pub ready: bool,
        pub route: ::std::string::String,
    }
    ///`ScientiaOperatorStatusSurfaceV1SnapshotSummary`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "hard_gate_failures",
    ///    "soft_gate_failures"
    ///  ],
    ///  "properties": {
    ///    "diagnostic_count": {
    ///      "type": "integer",
    ///      "minimum": 0.0
    ///    },
    ///    "hard_gate_failures": {
    ///      "type": "integer",
    ///      "minimum": 0.0
    ///    },
    ///    "soft_gate_failures": {
    ///      "type": "integer",
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaOperatorStatusSurfaceV1SnapshotSummary {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub diagnostic_count: ::std::option::Option<u64>,
        pub hard_gate_failures: u64,
        pub soft_gate_failures: u64,
    }
}

// --- contracts/scientia\publication-worthiness.schema.json ---
pub mod publication_worthiness_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///Machine-readable policy for classifying publication candidates as Publish / AskForEvidence / AbstainDoNotPublish. `venue_profiles` is currently advisory documentation for operator review; the evaluator enforces thresholds, weights, and red lines.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/publication-worthiness.schema.json",
    ///  "title": "Scientia Publication Worthiness Contract",
    ///  "description": "Machine-readable policy for classifying publication candidates as Publish / AskForEvidence / AbstainDoNotPublish. `venue_profiles` is currently advisory documentation for operator review; the evaluator enforces thresholds, weights, and red lines.",
    ///  "type": "object",
    ///  "required": [
    ///    "decision_labels",
    ///    "hard_red_lines",
    ///    "thresholds",
    ///    "venue_profiles",
    ///    "version",
    ///    "weights"
    ///  ],
    ///  "properties": {
    ///    "decision_labels": {
    ///      "type": "object",
    ///      "required": [
    ///        "abstain_do_not_publish",
    ///        "ask_for_evidence",
    ///        "publish"
    ///      ],
    ///      "properties": {
    ///        "abstain_do_not_publish": {
    ///          "const": "Abstain/DoNotPublish"
    ///        },
    ///        "ask_for_evidence": {
    ///          "const": "AskForEvidence"
    ///        },
    ///        "publish": {
    ///          "const": "Publish"
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "hard_red_lines": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "description",
    ///          "enabled",
    ///          "id"
    ///        ],
    ///        "properties": {
    ///          "description": {
    ///            "type": "string",
    ///            "minLength": 1
    ///          },
    ///          "enabled": {
    ///            "type": "boolean"
    ///          },
    ///          "id": {
    ///            "type": "string",
    ///            "pattern": "^[a-z0-9_\\-]+$"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      },
    ///      "minItems": 1
    ///    },
    ///    "thresholds": {
    ///      "type": "object",
    ///      "required": [
    ///        "abstain_score_max",
    ///        "ai_disclosure_compliance_exact",
    ///        "artifact_replayability_min",
    ///        "before_after_pair_integrity_min",
    ///        "claim_evidence_coverage_min",
    ///        "metadata_completeness_min",
    ///        "publish_score_min"
    ///      ],
    ///      "properties": {
    ///        "abstain_score_max": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "ai_disclosure_compliance_exact": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "artifact_replayability_min": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "before_after_pair_integrity_min": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "claim_evidence_coverage_min": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "metadata_completeness_min": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "publish_score_min": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "venue_profiles": {
    ///      "description": "Advisory venue notes only. Current runtime evaluation does not automatically enforce these required_checks.",
    ///      "type": "object",
    ///      "minProperties": 1,
    ///      "additionalProperties": {
    ///        "type": "object",
    ///        "required": [
    ///          "description",
    ///          "required_checks"
    ///        ],
    ///        "properties": {
    ///          "description": {
    ///            "type": "string",
    ///            "minLength": 1
    ///          },
    ///          "required_checks": {
    ///            "type": "array",
    ///            "items": {
    ///              "type": "string",
    ///              "minLength": 1
    ///            },
    ///            "minItems": 1
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "version": {
    ///      "type": "integer",
    ///      "minimum": 1.0
    ///    },
    ///    "weights": {
    ///      "type": "object",
    ///      "required": [
    ///        "epistemic",
    ///        "metadata_policy",
    ///        "novelty",
    ///        "reliability",
    ///        "reproducibility"
    ///      ],
    ///      "properties": {
    ///        "epistemic": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "metadata_policy": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "novelty": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "reliability": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "reproducibility": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaPublicationWorthinessContract {
        pub decision_labels: ScientiaPublicationWorthinessContractDecisionLabels,
        pub hard_red_lines: ::std::vec::Vec<
            ScientiaPublicationWorthinessContractHardRedLinesItem,
        >,
        pub thresholds: ScientiaPublicationWorthinessContractThresholds,
        ///Advisory venue notes only. Current runtime evaluation does not automatically enforce these required_checks.
        pub venue_profiles: ::std::collections::HashMap<
            ::std::string::String,
            ScientiaPublicationWorthinessContractVenueProfilesValue,
        >,
        pub version: ::std::num::NonZeroU64,
        pub weights: ScientiaPublicationWorthinessContractWeights,
    }
    ///`ScientiaPublicationWorthinessContractDecisionLabels`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "abstain_do_not_publish",
    ///    "ask_for_evidence",
    ///    "publish"
    ///  ],
    ///  "properties": {
    ///    "abstain_do_not_publish": {
    ///      "const": "Abstain/DoNotPublish"
    ///    },
    ///    "ask_for_evidence": {
    ///      "const": "AskForEvidence"
    ///    },
    ///    "publish": {
    ///      "const": "Publish"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaPublicationWorthinessContractDecisionLabels {
        pub abstain_do_not_publish: ::serde_json::Value,
        pub ask_for_evidence: ::serde_json::Value,
        pub publish: ::serde_json::Value,
    }
    ///`ScientiaPublicationWorthinessContractHardRedLinesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "description",
    ///    "enabled",
    ///    "id"
    ///  ],
    ///  "properties": {
    ///    "description": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "enabled": {
    ///      "type": "boolean"
    ///    },
    ///    "id": {
    ///      "type": "string",
    ///      "pattern": "^[a-z0-9_\\-]+$"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaPublicationWorthinessContractHardRedLinesItem {
        pub description: ScientiaPublicationWorthinessContractHardRedLinesItemDescription,
        pub enabled: bool,
        pub id: ScientiaPublicationWorthinessContractHardRedLinesItemId,
    }
    ///`ScientiaPublicationWorthinessContractHardRedLinesItemDescription`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaPublicationWorthinessContractHardRedLinesItemDescription(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaPublicationWorthinessContractHardRedLinesItemDescription {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<
        ScientiaPublicationWorthinessContractHardRedLinesItemDescription,
    > for ::std::string::String {
        fn from(
            value: ScientiaPublicationWorthinessContractHardRedLinesItemDescription,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaPublicationWorthinessContractHardRedLinesItemDescription {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaPublicationWorthinessContractHardRedLinesItemDescription {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaPublicationWorthinessContractHardRedLinesItemDescription {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaPublicationWorthinessContractHardRedLinesItemDescription {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaPublicationWorthinessContractHardRedLinesItemDescription {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaPublicationWorthinessContractHardRedLinesItemId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "pattern": "^[a-z0-9_\\-]+$"
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaPublicationWorthinessContractHardRedLinesItemId(
        ::std::string::String,
    );
    impl ::std::ops::Deref for ScientiaPublicationWorthinessContractHardRedLinesItemId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaPublicationWorthinessContractHardRedLinesItemId>
    for ::std::string::String {
        fn from(value: ScientiaPublicationWorthinessContractHardRedLinesItemId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaPublicationWorthinessContractHardRedLinesItemId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> = ::std::sync::LazyLock::new(||
            { ::regress::Regex::new("^[a-z0-9_\\-]+$").unwrap() });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[a-z0-9_\\-]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaPublicationWorthinessContractHardRedLinesItemId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaPublicationWorthinessContractHardRedLinesItemId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaPublicationWorthinessContractHardRedLinesItemId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaPublicationWorthinessContractHardRedLinesItemId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaPublicationWorthinessContractThresholds`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "abstain_score_max",
    ///    "ai_disclosure_compliance_exact",
    ///    "artifact_replayability_min",
    ///    "before_after_pair_integrity_min",
    ///    "claim_evidence_coverage_min",
    ///    "metadata_completeness_min",
    ///    "publish_score_min"
    ///  ],
    ///  "properties": {
    ///    "abstain_score_max": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "ai_disclosure_compliance_exact": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "artifact_replayability_min": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "before_after_pair_integrity_min": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "claim_evidence_coverage_min": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "metadata_completeness_min": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "publish_score_min": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaPublicationWorthinessContractThresholds {
        pub abstain_score_max: f64,
        pub ai_disclosure_compliance_exact: f64,
        pub artifact_replayability_min: f64,
        pub before_after_pair_integrity_min: f64,
        pub claim_evidence_coverage_min: f64,
        pub metadata_completeness_min: f64,
        pub publish_score_min: f64,
    }
    ///`ScientiaPublicationWorthinessContractVenueProfilesValue`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "description",
    ///    "required_checks"
    ///  ],
    ///  "properties": {
    ///    "description": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "required_checks": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string",
    ///        "minLength": 1
    ///      },
    ///      "minItems": 1
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaPublicationWorthinessContractVenueProfilesValue {
        pub description: ScientiaPublicationWorthinessContractVenueProfilesValueDescription,
        pub required_checks: ::std::vec::Vec<
            ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem,
        >,
    }
    ///`ScientiaPublicationWorthinessContractVenueProfilesValueDescription`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaPublicationWorthinessContractVenueProfilesValueDescription(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaPublicationWorthinessContractVenueProfilesValueDescription {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<
        ScientiaPublicationWorthinessContractVenueProfilesValueDescription,
    > for ::std::string::String {
        fn from(
            value: ScientiaPublicationWorthinessContractVenueProfilesValueDescription,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaPublicationWorthinessContractVenueProfilesValueDescription {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaPublicationWorthinessContractVenueProfilesValueDescription {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaPublicationWorthinessContractVenueProfilesValueDescription {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaPublicationWorthinessContractVenueProfilesValueDescription {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaPublicationWorthinessContractVenueProfilesValueDescription {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem(
        ::std::string::String,
    );
    impl ::std::ops::Deref
    for ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<
        ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem,
    > for ::std::string::String {
        fn from(
            value: ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem,
        ) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr
    for ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str>
    for ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de>
    for ScientiaPublicationWorthinessContractVenueProfilesValueRequiredChecksItem {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaPublicationWorthinessContractWeights`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "epistemic",
    ///    "metadata_policy",
    ///    "novelty",
    ///    "reliability",
    ///    "reproducibility"
    ///  ],
    ///  "properties": {
    ///    "epistemic": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "metadata_policy": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "novelty": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "reliability": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "reproducibility": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaPublicationWorthinessContractWeights {
        pub epistemic: f64,
        pub metadata_policy: f64,
        pub novelty: f64,
        pub reliability: f64,
        pub reproducibility: f64,
    }
}

// --- contracts/scientia\research-snapshot.v1.schema.json ---
pub mod research_snapshot_v1_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///Auditable recomputation snapshot for worthiness and metadata readiness diagnostics.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/research-snapshot.v1.schema.json",
    ///  "title": "SCIENTIA research snapshot v1",
    ///  "description": "Auditable recomputation snapshot for worthiness and metadata readiness diagnostics.",
    ///  "type": "object",
    ///  "required": [
    ///    "computed_at_ms",
    ///    "coverage",
    ///    "policy_profile",
    ///    "publication_id",
    ///    "signals",
    ///    "version"
    ///  ],
    ///  "properties": {
    ///    "citation_verification": {
    ///      "type": "object",
    ///      "properties": {
    ///        "confidence": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "unresolved_count": {
    ///          "type": "integer",
    ///          "minimum": 0.0
    ///        },
    ///        "verified_count": {
    ///          "type": "integer",
    ///          "minimum": 0.0
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "computed_at_ms": {
    ///      "type": "integer",
    ///      "minimum": 0.0
    ///    },
    ///    "coverage": {
    ///      "type": "object",
    ///      "required": [
    ///        "metadata_recommended",
    ///        "metadata_required"
    ///      ],
    ///      "properties": {
    ///        "metadata_recommended": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        },
    ///        "metadata_required": {
    ///          "type": "number",
    ///          "maximum": 1.0,
    ///          "minimum": 0.0
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "external_signal_provenance": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "confidence",
    ///          "retrieved_at_ms",
    ///          "source"
    ///        ],
    ///        "properties": {
    ///          "confidence": {
    ///            "type": "number",
    ///            "maximum": 1.0,
    ///            "minimum": 0.0
    ///          },
    ///          "notes": {
    ///            "type": "string"
    ///          },
    ///          "retrieved_at_ms": {
    ///            "type": "integer",
    ///            "minimum": 0.0
    ///          },
    ///          "source": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "policy_profile": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "previous_snapshot_hash": {
    ///      "type": "string"
    ///    },
    ///    "publication_id": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "signals": {
    ///      "type": "object",
    ///      "required": [
    ///        "diagnostic",
    ///        "hard_gate",
    ///        "soft_gate"
    ///      ],
    ///      "properties": {
    ///        "diagnostic": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "hard_gate": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "soft_gate": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        }
    ///      },
    ///      "additionalProperties": false
    ///    },
    ///    "version": {
    ///      "type": "string",
    ///      "const": "v1"
    ///    }
    ///  },
    ///  "additionalProperties": false,
    ///  "x-vox-version": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaResearchSnapshotV1 {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub citation_verification: ::std::option::Option<
            ScientiaResearchSnapshotV1CitationVerification,
        >,
        pub computed_at_ms: u64,
        pub coverage: ScientiaResearchSnapshotV1Coverage,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub external_signal_provenance: ::std::vec::Vec<
            ScientiaResearchSnapshotV1ExternalSignalProvenanceItem,
        >,
        pub policy_profile: ScientiaResearchSnapshotV1PolicyProfile,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub previous_snapshot_hash: ::std::option::Option<::std::string::String>,
        pub publication_id: ScientiaResearchSnapshotV1PublicationId,
        pub signals: ScientiaResearchSnapshotV1Signals,
        pub version: ::std::string::String,
    }
    ///`ScientiaResearchSnapshotV1CitationVerification`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "confidence": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "unresolved_count": {
    ///      "type": "integer",
    ///      "minimum": 0.0
    ///    },
    ///    "verified_count": {
    ///      "type": "integer",
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaResearchSnapshotV1CitationVerification {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub confidence: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub unresolved_count: ::std::option::Option<u64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub verified_count: ::std::option::Option<u64>,
    }
    impl ::std::default::Default for ScientiaResearchSnapshotV1CitationVerification {
        fn default() -> Self {
            Self {
                confidence: Default::default(),
                unresolved_count: Default::default(),
                verified_count: Default::default(),
            }
        }
    }
    ///`ScientiaResearchSnapshotV1Coverage`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "metadata_recommended",
    ///    "metadata_required"
    ///  ],
    ///  "properties": {
    ///    "metadata_recommended": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "metadata_required": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaResearchSnapshotV1Coverage {
        pub metadata_recommended: f64,
        pub metadata_required: f64,
    }
    ///`ScientiaResearchSnapshotV1ExternalSignalProvenanceItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "confidence",
    ///    "retrieved_at_ms",
    ///    "source"
    ///  ],
    ///  "properties": {
    ///    "confidence": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    },
    ///    "notes": {
    ///      "type": "string"
    ///    },
    ///    "retrieved_at_ms": {
    ///      "type": "integer",
    ///      "minimum": 0.0
    ///    },
    ///    "source": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaResearchSnapshotV1ExternalSignalProvenanceItem {
        pub confidence: f64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notes: ::std::option::Option<::std::string::String>,
        pub retrieved_at_ms: u64,
        pub source: ::std::string::String,
    }
    ///`ScientiaResearchSnapshotV1PolicyProfile`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaResearchSnapshotV1PolicyProfile(::std::string::String);
    impl ::std::ops::Deref for ScientiaResearchSnapshotV1PolicyProfile {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaResearchSnapshotV1PolicyProfile>
    for ::std::string::String {
        fn from(value: ScientiaResearchSnapshotV1PolicyProfile) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaResearchSnapshotV1PolicyProfile {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaResearchSnapshotV1PolicyProfile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaResearchSnapshotV1PolicyProfile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaResearchSnapshotV1PolicyProfile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaResearchSnapshotV1PolicyProfile {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaResearchSnapshotV1PublicationId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ScientiaResearchSnapshotV1PublicationId(::std::string::String);
    impl ::std::ops::Deref for ScientiaResearchSnapshotV1PublicationId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ScientiaResearchSnapshotV1PublicationId>
    for ::std::string::String {
        fn from(value: ScientiaResearchSnapshotV1PublicationId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ScientiaResearchSnapshotV1PublicationId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaResearchSnapshotV1PublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaResearchSnapshotV1PublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaResearchSnapshotV1PublicationId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ScientiaResearchSnapshotV1PublicationId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ScientiaResearchSnapshotV1Signals`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "diagnostic",
    ///    "hard_gate",
    ///    "soft_gate"
    ///  ],
    ///  "properties": {
    ///    "diagnostic": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "hard_gate": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "soft_gate": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaResearchSnapshotV1Signals {
        pub diagnostic: ::std::vec::Vec<::std::string::String>,
        pub hard_gate: ::std::vec::Vec<::std::string::String>,
        pub soft_gate: ::std::vec::Vec<::std::string::String>,
    }
}

// --- contracts/scientia\scientia-evidence-graph.schema.json ---
pub mod scientia_evidence_graph_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///Shared evidence bundle for scholarly + social automation. Optional fields extend over time.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/scientia-evidence-graph.schema.json",
    ///  "title": "metadata_json.scientia_evidence graph",
    ///  "description": "Shared evidence bundle for scholarly + social automation. Optional fields extend over time.",
    ///  "type": "object",
    ///  "properties": {
    ///    "autofill_provenance": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "properties": {
    ///          "facet": {
    ///            "type": "string"
    ///          },
    ///          "notes": {
    ///            "type": "string"
    ///          },
    ///          "origin": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "benchmark": {
    ///      "type": "object"
    ///    },
    ///    "benchmark_pair_report_repo_relative": {
    ///      "type": "string"
    ///    },
    ///    "candidate_note": {
    ///      "type": "string"
    ///    },
    ///    "discovery_signals": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "additionalProperties": true
    ///      }
    ///    },
    ///    "doc_section_hints": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "properties": {
    ///          "heading_level": {
    ///            "type": "integer",
    ///            "maximum": 6.0,
    ///            "minimum": 1.0
    ///          },
    ///          "line": {
    ///            "type": "integer",
    ///            "minimum": 1.0
    ///          },
    ///          "slug": {
    ///            "type": "string"
    ///          },
    ///          "title": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "draft_preparation": {
    ///      "type": "object"
    ///    },
    ///    "eval_gate": {
    ///      "type": "object"
    ///    },
    ///    "eval_gate_report_repo_relative": {
    ///      "type": "string"
    ///    },
    ///    "eval_gate_run_dir_repo_relative": {
    ///      "type": "string"
    ///    },
    ///    "human_ai_disclosure_complete": {
    ///      "type": "boolean"
    ///    },
    ///    "human_meaningful_advance": {
    ///      "type": "boolean"
    ///    },
    ///    "linked_doc_repo_relatives": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "mens_scorecard_repo_relative": {
    ///      "type": "string"
    ///    },
    ///    "reproducibility_manifest_repo_relative": {
    ///      "type": "string"
    ///    },
    ///    "signal_conflicts": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "summary"
    ///        ],
    ///        "properties": {
    ///          "codes": {
    ///            "type": "array",
    ///            "items": {
    ///              "type": "string"
    ///            }
    ///          },
    ///          "summary": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "additionalProperties": false
    ///      }
    ///    },
    ///    "socrates_aggregate": {
    ///      "type": "object"
    ///    },
    ///    "trust_rollup_repo_relative": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct MetadataJsonScientiaEvidenceGraph {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub autofill_provenance: ::std::vec::Vec<
            MetadataJsonScientiaEvidenceGraphAutofillProvenanceItem,
        >,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub benchmark: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub benchmark_pair_report_repo_relative: ::std::option::Option<
            ::std::string::String,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub candidate_note: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub discovery_signals: ::std::vec::Vec<
            ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        >,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub doc_section_hints: ::std::vec::Vec<
            MetadataJsonScientiaEvidenceGraphDocSectionHintsItem,
        >,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub draft_preparation: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub eval_gate: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub eval_gate_report_repo_relative: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub eval_gate_run_dir_repo_relative: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub human_ai_disclosure_complete: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub human_meaningful_advance: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub linked_doc_repo_relatives: ::std::vec::Vec<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub mens_scorecard_repo_relative: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub reproducibility_manifest_repo_relative: ::std::option::Option<
            ::std::string::String,
        >,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub signal_conflicts: ::std::vec::Vec<
            MetadataJsonScientiaEvidenceGraphSignalConflictsItem,
        >,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub socrates_aggregate: ::serde_json::Map<
            ::std::string::String,
            ::serde_json::Value,
        >,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub trust_rollup_repo_relative: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for MetadataJsonScientiaEvidenceGraph {
        fn default() -> Self {
            Self {
                autofill_provenance: Default::default(),
                benchmark: Default::default(),
                benchmark_pair_report_repo_relative: Default::default(),
                candidate_note: Default::default(),
                discovery_signals: Default::default(),
                doc_section_hints: Default::default(),
                draft_preparation: Default::default(),
                eval_gate: Default::default(),
                eval_gate_report_repo_relative: Default::default(),
                eval_gate_run_dir_repo_relative: Default::default(),
                human_ai_disclosure_complete: Default::default(),
                human_meaningful_advance: Default::default(),
                linked_doc_repo_relatives: Default::default(),
                mens_scorecard_repo_relative: Default::default(),
                reproducibility_manifest_repo_relative: Default::default(),
                signal_conflicts: Default::default(),
                socrates_aggregate: Default::default(),
                trust_rollup_repo_relative: Default::default(),
            }
        }
    }
    ///`MetadataJsonScientiaEvidenceGraphAutofillProvenanceItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "facet": {
    ///      "type": "string"
    ///    },
    ///    "notes": {
    ///      "type": "string"
    ///    },
    ///    "origin": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct MetadataJsonScientiaEvidenceGraphAutofillProvenanceItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub facet: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub notes: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub origin: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default
    for MetadataJsonScientiaEvidenceGraphAutofillProvenanceItem {
        fn default() -> Self {
            Self {
                facet: Default::default(),
                notes: Default::default(),
                origin: Default::default(),
            }
        }
    }
    ///`MetadataJsonScientiaEvidenceGraphDocSectionHintsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "heading_level": {
    ///      "type": "integer",
    ///      "maximum": 6.0,
    ///      "minimum": 1.0
    ///    },
    ///    "line": {
    ///      "type": "integer",
    ///      "minimum": 1.0
    ///    },
    ///    "slug": {
    ///      "type": "string"
    ///    },
    ///    "title": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct MetadataJsonScientiaEvidenceGraphDocSectionHintsItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub heading_level: ::std::option::Option<::std::num::NonZeroU64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub line: ::std::option::Option<::std::num::NonZeroU64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub slug: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for MetadataJsonScientiaEvidenceGraphDocSectionHintsItem {
        fn default() -> Self {
            Self {
                heading_level: Default::default(),
                line: Default::default(),
                slug: Default::default(),
                title: Default::default(),
            }
        }
    }
    ///`MetadataJsonScientiaEvidenceGraphSignalConflictsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "summary"
    ///  ],
    ///  "properties": {
    ///    "codes": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "summary": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct MetadataJsonScientiaEvidenceGraphSignalConflictsItem {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub codes: ::std::vec::Vec<::std::string::String>,
        pub summary: ::std::string::String,
    }
}

// --- contracts/scientia\worthiness-signals.v2.schema.json ---
pub mod worthiness_signals_v2_schema {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(
                &self,
                f: &mut ::std::fmt::Formatter<'_>,
            ) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///`ActionItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "action",
    ///    "id",
    ///    "priority"
    ///  ],
    ///  "properties": {
    ///    "action": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "id": {
    ///      "type": "string",
    ///      "pattern": "^[a-z0-9_\\-]+$"
    ///    },
    ///    "priority": {
    ///      "type": "integer",
    ///      "maximum": 999.0,
    ///      "minimum": 1.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ActionItem {
        pub action: ActionItemAction,
        pub id: ActionItemId,
        pub priority: ::std::num::NonZeroU64,
    }
    ///`ActionItemAction`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ActionItemAction(::std::string::String);
    impl ::std::ops::Deref for ActionItemAction {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ActionItemAction> for ::std::string::String {
        fn from(value: ActionItemAction) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ActionItemAction {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ActionItemAction {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for ActionItemAction {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ActionItemAction {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ActionItemAction {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`ActionItemId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "pattern": "^[a-z0-9_\\-]+$"
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct ActionItemId(::std::string::String);
    impl ::std::ops::Deref for ActionItemId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<ActionItemId> for ::std::string::String {
        fn from(value: ActionItemId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for ActionItemId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> = ::std::sync::LazyLock::new(||
            { ::regress::Regex::new("^[a-z0-9_\\-]+$").unwrap() });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[a-z0-9_\\-]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for ActionItemId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for ActionItemId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for ActionItemId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for ActionItemId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///Profile-aware signal output contract for publication-worthiness v2.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "$id": "https://vox-lang.org/contracts/scientia/worthiness-signals.v2.schema.json",
    ///  "title": "SCIENTIA worthiness signals v2",
    ///  "description": "Profile-aware signal output contract for publication-worthiness v2.",
    ///  "type": "object",
    ///  "required": [
    ///    "diagnostic",
    ///    "hard_gate",
    ///    "next_actions",
    ///    "profile",
    ///    "soft_gate",
    ///    "version"
    ///  ],
    ///  "properties": {
    ///    "diagnostic": {
    ///      "$ref": "#/$defs/signalArray"
    ///    },
    ///    "hard_gate": {
    ///      "$ref": "#/$defs/signalArray"
    ///    },
    ///    "next_actions": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/$defs/actionItem"
    ///      }
    ///    },
    ///    "profile": {
    ///      "type": "string",
    ///      "enum": [
    ///        "journal",
    ///        "preprint",
    ///        "repository",
    ///        "social"
    ///      ]
    ///    },
    ///    "soft_gate": {
    ///      "$ref": "#/$defs/signalArray"
    ///    },
    ///    "version": {
    ///      "type": "string",
    ///      "const": "v2"
    ///    }
    ///  },
    ///  "additionalProperties": false,
    ///  "x-vox-version": 2
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct ScientiaWorthinessSignalsV2 {
        pub diagnostic: SignalArray,
        pub hard_gate: SignalArray,
        pub next_actions: ::std::vec::Vec<ActionItem>,
        pub profile: ScientiaWorthinessSignalsV2Profile,
        pub soft_gate: SignalArray,
        pub version: ::std::string::String,
    }
    ///`ScientiaWorthinessSignalsV2Profile`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "journal",
    ///    "preprint",
    ///    "repository",
    ///    "social"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd
    )]
    pub enum ScientiaWorthinessSignalsV2Profile {
        #[serde(rename = "journal")]
        Journal,
        #[serde(rename = "preprint")]
        Preprint,
        #[serde(rename = "repository")]
        Repository,
        #[serde(rename = "social")]
        Social,
    }
    impl ::std::fmt::Display for ScientiaWorthinessSignalsV2Profile {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Journal => f.write_str("journal"),
                Self::Preprint => f.write_str("preprint"),
                Self::Repository => f.write_str("repository"),
                Self::Social => f.write_str("social"),
            }
        }
    }
    impl ::std::str::FromStr for ScientiaWorthinessSignalsV2Profile {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "journal" => Ok(Self::Journal),
                "preprint" => Ok(Self::Preprint),
                "repository" => Ok(Self::Repository),
                "social" => Ok(Self::Social),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for ScientiaWorthinessSignalsV2Profile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
    for ScientiaWorthinessSignalsV2Profile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
    for ScientiaWorthinessSignalsV2Profile {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`SignalArray`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "array",
    ///  "items": {
    ///    "type": "object",
    ///    "required": [
    ///      "id",
    ///      "passed",
    ///      "reason_code",
    ///      "score"
    ///    ],
    ///    "properties": {
    ///      "details": {
    ///        "type": "string"
    ///      },
    ///      "id": {
    ///        "type": "string",
    ///        "pattern": "^[a-z0-9_\\-]+$"
    ///      },
    ///      "passed": {
    ///        "type": "boolean"
    ///      },
    ///      "reason_code": {
    ///        "type": "string",
    ///        "minLength": 1
    ///      },
    ///      "score": {
    ///        "type": "number",
    ///        "maximum": 1.0,
    ///        "minimum": 0.0
    ///      }
    ///    },
    ///    "additionalProperties": false
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct SignalArray(pub ::std::vec::Vec<SignalArrayItem>);
    impl ::std::ops::Deref for SignalArray {
        type Target = ::std::vec::Vec<SignalArrayItem>;
        fn deref(&self) -> &::std::vec::Vec<SignalArrayItem> {
            &self.0
        }
    }
    impl ::std::convert::From<SignalArray> for ::std::vec::Vec<SignalArrayItem> {
        fn from(value: SignalArray) -> Self {
            value.0
        }
    }
    impl ::std::convert::From<::std::vec::Vec<SignalArrayItem>> for SignalArray {
        fn from(value: ::std::vec::Vec<SignalArrayItem>) -> Self {
            Self(value)
        }
    }
    ///`SignalArrayItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "id",
    ///    "passed",
    ///    "reason_code",
    ///    "score"
    ///  ],
    ///  "properties": {
    ///    "details": {
    ///      "type": "string"
    ///    },
    ///    "id": {
    ///      "type": "string",
    ///      "pattern": "^[a-z0-9_\\-]+$"
    ///    },
    ///    "passed": {
    ///      "type": "boolean"
    ///    },
    ///    "reason_code": {
    ///      "type": "string",
    ///      "minLength": 1
    ///    },
    ///    "score": {
    ///      "type": "number",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    }
    ///  },
    ///  "additionalProperties": false
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct SignalArrayItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub details: ::std::option::Option<::std::string::String>,
        pub id: SignalArrayItemId,
        pub passed: bool,
        pub reason_code: SignalArrayItemReasonCode,
        pub score: f64,
    }
    ///`SignalArrayItemId`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "pattern": "^[a-z0-9_\\-]+$"
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SignalArrayItemId(::std::string::String);
    impl ::std::ops::Deref for SignalArrayItemId {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<SignalArrayItemId> for ::std::string::String {
        fn from(value: SignalArrayItemId) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for SignalArrayItemId {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            static PATTERN: ::std::sync::LazyLock<::regress::Regex> = ::std::sync::LazyLock::new(||
            { ::regress::Regex::new("^[a-z0-9_\\-]+$").unwrap() });
            if PATTERN.find(value).is_none() {
                return Err("doesn't match pattern \"^[a-z0-9_\\-]+$\"".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for SignalArrayItemId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for SignalArrayItemId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for SignalArrayItemId {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for SignalArrayItemId {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`SignalArrayItemReasonCode`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "minLength": 1
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct SignalArrayItemReasonCode(::std::string::String);
    impl ::std::ops::Deref for SignalArrayItemReasonCode {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<SignalArrayItemReasonCode> for ::std::string::String {
        fn from(value: SignalArrayItemReasonCode) -> Self {
            value.0
        }
    }
    impl ::std::str::FromStr for SignalArrayItemReasonCode {
        type Err = self::error::ConversionError;
        fn from_str(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() < 1usize {
                return Err("shorter than 1 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for SignalArrayItemReasonCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &str,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for SignalArrayItemReasonCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for SignalArrayItemReasonCode {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for SignalArrayItemReasonCode {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
}

