//! What each T4127 factor the engine returns means, in this project's own
//! words. The engine names them; this file explains them.

/// The meaning of the factor `name`, when the engine names it.
pub fn meaning(name: &str) -> Option<&'static str> {
    MEANINGS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, meaning)| *meaning)
}

const MEANINGS: [(&str, &str); 28] = [
    ("P", "Pay periods in the year."),
    (
        "PM",
        "Months of the year in which the employee contributes to CPP.",
    ),
    (
        "C",
        "The employee's CPP contribution on this pay, base and first additional.",
    ),
    (
        "C2",
        "The employee's second additional CPP contribution (CPP2) on this pay.",
    ),
    ("EI", "The employee's EI premium on this pay."),
    (
        "F5",
        "The part of this pay's CPP that is deducted from income before tax: the first additional contribution and CPP2.",
    ),
    (
        "F5A",
        "F5 for the regular pay; with no bonus it is F5 whenever there are pensionable earnings.",
    ),
    (
        "A",
        "Annual taxable income: pay periods times this pay, less F5A.",
    ),
    ("R", "The federal tax rate for the bracket A falls in."),
    ("K", "The federal constant for the bracket A falls in."),
    (
        "K1",
        "The federal credit for the federal claim amount, at the lowest federal rate.",
    ),
    (
        "K2",
        "The federal credit for a year of base CPP contributions and EI premiums at this pay.",
    ),
    ("K4", "The federal credit for the Canada employment amount."),
    (
        "T3",
        "Annual basic federal tax: R times A, less K and the federal credits, not below zero.",
    ),
    (
        "T1",
        "Annual federal tax; with no labour-sponsored funds credit it is T3.",
    ),
    ("V", "The Ontario tax rate for the bracket A falls in."),
    ("KP", "The Ontario constant for the bracket A falls in."),
    (
        "K1P",
        "The Ontario credit for the Ontario claim amount, at the lowest Ontario rate.",
    ),
    (
        "K2P",
        "The Ontario credit for a year of base CPP contributions and EI premiums at this pay.",
    ),
    (
        "K4P",
        "The provincial credit for an employment amount, where the province has one.",
    ),
    (
        "K5P",
        "The provincial supplemental credit, where the province has one.",
    ),
    (
        "T4",
        "Annual basic Ontario tax: V times A, less KP and the Ontario credits, not below zero.",
    ),
    ("V1", "The Ontario surtax on T4."),
    ("V2", "The Ontario Health Premium on A."),
    (
        "S",
        "The Ontario tax reduction, which lowers or removes Ontario tax on lower incomes.",
    ),
    (
        "T2",
        "Annual Ontario tax: T4 plus V1 and V2, less S, not below zero.",
    ),
    (
        "L",
        "Additional tax per pay asked for on a TD1; none on this page.",
    ),
    (
        "T",
        "Income tax deducted from this pay: federal and Ontario tax for the year, each divided by P and rounded to the cent, plus L.",
    ),
];
