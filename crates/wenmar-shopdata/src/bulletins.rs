//! Manufacturer communications NHTSA publishes: service bulletins,
//! service campaigns, warranty extensions, over-the-air updates.
//!
//! The file holds NHTSA's index of each communication and its summary.
//! The document itself is the manufacturer's and is never stored here.

use std::fmt;

/// What kind of communication a bulletin is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BulletinType {
    ServiceBulletin,
    ServiceCampaign,
    WarrantyExtension,
    OverTheAir,
    Emissions,
    Other,
}

impl BulletinType {
    pub const ALL: [BulletinType; 6] = [
        BulletinType::ServiceBulletin,
        BulletinType::ServiceCampaign,
        BulletinType::WarrantyExtension,
        BulletinType::OverTheAir,
        BulletinType::Emissions,
        BulletinType::Other,
    ];

    /// The `type` column, and the word a caller filters by.
    pub fn id(self) -> &'static str {
        match self {
            BulletinType::ServiceBulletin => "service_bulletin",
            BulletinType::ServiceCampaign => "service_campaign",
            BulletinType::WarrantyExtension => "warranty_extension",
            BulletinType::OverTheAir => "over_the_air",
            BulletinType::Emissions => "emissions",
            BulletinType::Other => "other",
        }
    }

    /// The name a page shows.
    pub fn label(self) -> &'static str {
        match self {
            BulletinType::ServiceBulletin => "Service bulletin",
            BulletinType::ServiceCampaign => "Service campaign",
            BulletinType::WarrantyExtension => "Warranty extension",
            BulletinType::OverTheAir => "Over-the-air update",
            BulletinType::Emissions => "Emissions",
            BulletinType::Other => "Other",
        }
    }

    /// The type as NHTSA's files spell it.
    pub fn source_text(self) -> &'static str {
        match self {
            BulletinType::ServiceBulletin => "Service Bulletin/Repair Instructions",
            BulletinType::ServiceCampaign => "Service Campaign",
            BulletinType::WarrantyExtension => "Warranty Program/Extension",
            BulletinType::OverTheAir => "Over The Air",
            BulletinType::Emissions => "Emissions",
            BulletinType::Other => "Other",
        }
    }

    /// Reads the `type` column. `None` for anything else.
    pub fn parse(id: &str) -> Option<BulletinType> {
        BulletinType::ALL.into_iter().find(|kind| kind.id() == id)
    }

    /// Reads NHTSA's spelling, in either case and with or without spaces
    /// around the slash. `None` for a type this build does not know.
    pub fn from_source(text: &str) -> Option<BulletinType> {
        let squeezed = |text: &str| -> String {
            text.chars()
                .filter(|character| !character.is_whitespace())
                .flat_map(char::to_lowercase)
                .collect()
        };
        let wanted = squeezed(text);
        BulletinType::ALL
            .into_iter()
            .find(|kind| squeezed(kind.source_text()) == wanted)
    }
}

impl fmt::Display for BulletinType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_type_has_an_id_a_label_and_nhtsas_spelling() {
        let ids: Vec<&str> = BulletinType::ALL.iter().map(|kind| kind.id()).collect();
        assert_eq!(
            ids,
            [
                "service_bulletin",
                "service_campaign",
                "warranty_extension",
                "over_the_air",
                "emissions",
                "other"
            ]
        );
        for kind in BulletinType::ALL {
            assert_eq!(BulletinType::parse(kind.id()), Some(kind));
            assert_eq!(BulletinType::from_source(kind.source_text()), Some(kind));
            assert_eq!(kind.to_string(), kind.id());
            assert!(!kind.label().is_empty());
        }
        // TSBS.txt writes two of them with spaces around the slash.
        assert_eq!(
            BulletinType::from_source("Service Bulletin / Repair Instructions"),
            Some(BulletinType::ServiceBulletin)
        );
        assert_eq!(
            BulletinType::from_source("WARRANTY PROGRAM / EXTENSION"),
            Some(BulletinType::WarrantyExtension)
        );
        for bad in ["", "Recall", "service_bulletin"] {
            assert_eq!(BulletinType::from_source(bad), None, "{bad}");
        }
        for bad in ["", "Service Campaign", "ota"] {
            assert_eq!(BulletinType::parse(bad), None, "{bad}");
        }
    }
}
