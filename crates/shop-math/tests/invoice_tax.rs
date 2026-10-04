use shop_math::invoice_tax::{
    FeeLine, Invoice, InvoiceField, InvoiceTaxError, Tax, TaxLine, Tires, invoice_tax,
};
use shop_math::{Money, Percent, TooLarge};

fn cents(cents: i64) -> Money {
    Money::from_cents(cents)
}

/// A tax on everything: labour, parts, shop supplies and the tire fee.
fn on_everything(thousandths: i64) -> Tax {
    Tax {
        rate: Percent::from_thousandths(thousandths),
        on_labour: true,
        on_parts: true,
        on_supplies: true,
        on_tire_fee: true,
    }
}

fn tires(fee: i64, count: u32) -> Tires {
    Tires {
        fee: cents(fee),
        count,
    }
}

/// Labour 180.00, parts 640.00 and shop supplies 14.37: 834.37 before
/// fees and taxes.
fn invoice(tires: &[Tires]) -> Invoice<'_> {
    Invoice {
        labour: cents(18_000),
        parts: cents(64_000),
        supplies: cents(1_437),
        tires,
    }
}

#[test]
fn the_subtotal_the_fee_lines_the_tax_lines_and_the_total() {
    // Four tires at 6.50 and one at 14.00: 26.00 and 14.00 of fees.
    // Each tax is on 834.37 + 40.00 = 874.37.
    // 5% of 874.37 is 43.7185, shown as 43.72.
    // 7% of 874.37 is 61.2059, shown as 61.21.
    // Total: 834.37 + 26.00 + 14.00 + 43.72 + 61.21 = 979.30.
    let tires = [tires(650, 4), tires(1_400, 1)];
    let taxes = [on_everything(5_000), on_everything(7_000)];
    let result = invoice_tax(&taxes, &invoice(&tires)).unwrap();
    assert_eq!(result.subtotal, cents(83_437));
    assert_eq!(
        result.fees,
        [
            FeeLine {
                fee: cents(650),
                count: 4,
                amount: cents(2_600)
            },
            FeeLine {
                fee: cents(1_400),
                count: 1,
                amount: cents(1_400)
            },
        ]
    );
    assert_eq!(
        result.taxes,
        [
            TaxLine {
                rate: Percent::whole(5),
                base: cents(87_437),
                amount: cents(4_372)
            },
            TaxLine {
                rate: Percent::whole(7),
                base: cents(87_437),
                amount: cents(6_121)
            },
        ]
    );
    assert_eq!(result.total, cents(97_930));
}

#[test]
fn a_second_tax_is_on_the_price_and_never_on_the_first_tax() {
    // 100.00 of parts, a tax of 5% and one of 9.975%: 5.00 and 9.975,
    // shown as 9.98. On the price with the first tax in it the second
    // would be 10.47; it is not.
    let taxes = [on_everything(5_000), on_everything(9_975)];
    let invoice = Invoice {
        labour: Money::ZERO,
        parts: cents(10_000),
        supplies: Money::ZERO,
        tires: &[],
    };
    let result = invoice_tax(&taxes, &invoice).unwrap();
    assert_eq!(result.taxes[0].amount, cents(500));
    assert_eq!(result.taxes[1].base, cents(10_000));
    assert_eq!(result.taxes[1].amount, cents(998));
    assert_eq!(result.total, cents(11_498));
    // The order the taxes are given in changes nothing.
    let turned = [taxes[1], taxes[0]];
    assert_eq!(invoice_tax(&turned, &invoice).unwrap().total, result.total);
}

#[test]
fn a_tax_is_calculated_only_on_what_it_applies_to() {
    // The second tax does not apply to the tire fee: it is on 834.37, and
    // the first is on 874.37.
    let not_on_fee = Tax {
        on_tire_fee: false,
        ..on_everything(6_000)
    };
    let tires = [tires(650, 4), tires(1_400, 1)];
    let result = invoice_tax(&[on_everything(5_000), not_on_fee], &invoice(&tires)).unwrap();
    assert_eq!(result.taxes[0].base, cents(87_437));
    assert_eq!(result.taxes[0].amount, cents(4_372));
    assert_eq!(result.taxes[1].base, cents(83_437));
    // 6% of 834.37 is 50.0622.
    assert_eq!(result.taxes[1].amount, cents(5_006));
    assert_eq!(result.total, cents(83_437 + 4_000 + 4_372 + 5_006));

    // Each of the other three switches takes its own amount out.
    for (tax, base) in [
        (
            Tax {
                on_labour: false,
                ..on_everything(5_000)
            },
            69_437,
        ),
        (
            Tax {
                on_parts: false,
                ..on_everything(5_000)
            },
            23_437,
        ),
        (
            Tax {
                on_supplies: false,
                ..on_everything(5_000)
            },
            86_000,
        ),
    ] {
        let result = invoice_tax(&[tax], &invoice(&tires)).unwrap();
        assert_eq!(result.taxes[0].base, cents(base));
    }
}

#[test]
fn a_tax_is_rounded_once_on_its_base_and_a_half_cent_goes_up() {
    for (base, thousandths, tax) in [
        // 5% of 0.10 is half a cent: 0.01.
        (10, 5_000, 1),
        // 5% of 0.09 is 0.0045: nothing.
        (9, 5_000, 0),
        // 5% of 0.30 is a cent and a half: 0.02.
        (30, 5_000, 2),
        // 9.975% of 200.00 is 19.95 exactly.
        (20_000, 9_975, 1_995),
        // 9.975% of 0.05 is 0.0049875: nothing. Of 0.06, 0.005985: 0.01.
        (5, 9_975, 0),
        (6, 9_975, 1),
        // 15% of 33.33 is 4.9995: 5.00.
        (3_333, 15_000, 500),
    ] {
        let invoice = Invoice {
            labour: cents(base),
            parts: Money::ZERO,
            supplies: Money::ZERO,
            tires: &[],
        };
        let result = invoice_tax(&[on_everything(thousandths)], &invoice).unwrap();
        assert_eq!(
            result.taxes[0].amount,
            cents(tax),
            "{base} at {thousandths}"
        );
        assert_eq!(result.total, cents(base + tax));
    }
    // Three lines of 0.10 are taxed together, 0.015 shown as 0.02, and not
    // line by line, which would make 0.03.
    let invoice = Invoice {
        labour: cents(10),
        parts: cents(10),
        supplies: cents(10),
        tires: &[],
    };
    let result = invoice_tax(&[on_everything(5_000)], &invoice).unwrap();
    assert_eq!(result.taxes[0].amount, cents(2));
}

#[test]
fn the_total_is_the_sum_of_the_lines_as_they_are_shown() {
    let tires = [tires(450, 3), tires(600, 0)];
    let taxes = [on_everything(5_000), on_everything(9_975)];
    let result = invoice_tax(&taxes, &invoice(&tires)).unwrap();
    let mut sum = result.subtotal.cents();
    sum += result
        .fees
        .iter()
        .map(|fee| fee.amount.cents())
        .sum::<i64>();
    sum += result
        .taxes
        .iter()
        .map(|tax| tax.amount.cents())
        .sum::<i64>();
    assert_eq!(result.total.cents(), sum);
    // A class with no tires is still a line, of nothing.
    assert_eq!(result.fees.len(), 2);
    assert_eq!(result.fees[1].amount, Money::ZERO);
}

#[test]
fn no_tax_and_no_tires_leave_the_subtotal() {
    let result = invoice_tax(&[], &invoice(&[])).unwrap();
    assert_eq!(result.subtotal, cents(83_437));
    assert!(result.fees.is_empty() && result.taxes.is_empty());
    assert_eq!(result.total, cents(83_437));
    // Tires alone, with nothing else on the invoice.
    let only = [tires(500, 4)];
    let invoice = Invoice {
        labour: Money::ZERO,
        parts: Money::ZERO,
        supplies: Money::ZERO,
        tires: &only,
    };
    let result = invoice_tax(&[on_everything(5_000)], &invoice).unwrap();
    assert_eq!(result.subtotal, Money::ZERO);
    assert_eq!(result.taxes[0].base, cents(2_000));
    assert_eq!(result.total, cents(2_100));
}

#[test]
fn an_invoice_of_nothing_is_refused_at_the_form() {
    let nothing = [tires(650, 0)];
    let invoice = Invoice {
        labour: Money::ZERO,
        parts: Money::ZERO,
        supplies: Money::ZERO,
        tires: &nothing,
    };
    let error = invoice_tax(&[on_everything(5_000)], &invoice).unwrap_err();
    assert_eq!(error, InvoiceTaxError::Nothing);
    assert_eq!(error.field(), None);
    assert_eq!(
        error.to_string(),
        "Enter labour, parts, shop supplies or a number of new tires."
    );
}

#[test]
fn a_figure_below_zero_is_refused_beside_its_field() {
    for (labour, parts, supplies, field) in [
        (-1, 0, 0, InvoiceField::Labour),
        (0, -1, 0, InvoiceField::Parts),
        (0, 0, -1, InvoiceField::Supplies),
    ] {
        let invoice = Invoice {
            labour: cents(labour),
            parts: cents(parts),
            supplies: cents(supplies),
            tires: &[],
        };
        let error = invoice_tax(&[], &invoice).unwrap_err();
        assert_eq!(error, InvoiceTaxError::BelowZero { field });
        assert_eq!(error.field(), Some(field));
        assert_eq!(error.to_string(), "This cannot be below zero.");
    }
}

#[test]
fn a_rate_or_a_fee_below_zero_is_refused() {
    let below = [tires(-650, 4)];
    let error = invoice_tax(&[], &invoice(&below)).unwrap_err();
    assert_eq!(error, InvoiceTaxError::Rule);
    assert_eq!(error.field(), None);
    assert_eq!(error.to_string(), "A rate or a fee is below zero.");
    let error = invoice_tax(&[on_everything(-5_000)], &invoice(&[])).unwrap_err();
    assert_eq!(error, InvoiceTaxError::Rule);
}

#[test]
fn nothing_overflows() {
    // The largest of every field a form accepts, a thousand tires in each
    // of two classes, and two taxes: it is worked out.
    let most = Money::MAX_INPUT;
    let many = [tires(1_400, 1_000), tires(650, 1_000)];
    let invoice = Invoice {
        labour: most,
        parts: most,
        supplies: most,
        tires: &many,
    };
    let taxes = [on_everything(5_000), on_everything(9_975)];
    let result = invoice_tax(&taxes, &invoice).unwrap();
    // 3 × 99,999,999.99 = 299,999,999.97; fees 14,000.00 + 6,500.00.
    assert_eq!(result.subtotal, cents(29_999_999_997));
    assert_eq!(result.taxes[0].base, cents(30_002_049_997));
    // 5% is 15,001,024.9985; 9.975% is 29,927,044.872...
    assert_eq!(result.taxes[0].amount, cents(1_500_102_500));
    assert_eq!(result.taxes[1].amount, cents(2_992_704_487));
    assert_eq!(result.total, cents(34_494_856_984));

    // Figures no form can send are refused, not wrapped.
    let huge = [tires(i64::MAX, u32::MAX)];
    let error = invoice_tax(&[], &invoice_of(&huge)).unwrap_err();
    assert_eq!(error, InvoiceTaxError::TooLarge(TooLarge));
    assert_eq!(error.to_string(), "The result is too large to show.");
    let wide = Invoice {
        labour: cents(i64::MAX),
        parts: cents(i64::MAX),
        supplies: Money::ZERO,
        tires: &[],
    };
    assert_eq!(
        invoice_tax(&[], &wide).unwrap_err(),
        InvoiceTaxError::TooLarge(TooLarge)
    );
    let steep = [on_everything(i64::MAX)];
    let tall = Invoice {
        labour: cents(i64::MAX),
        parts: Money::ZERO,
        supplies: Money::ZERO,
        tires: &[],
    };
    assert_eq!(
        invoice_tax(&steep, &tall).unwrap_err(),
        InvoiceTaxError::TooLarge(TooLarge)
    );
}

fn invoice_of(tires: &[Tires]) -> Invoice<'_> {
    Invoice {
        labour: cents(100),
        parts: Money::ZERO,
        supplies: Money::ZERO,
        tires,
    }
}
