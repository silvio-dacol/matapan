fn main() {
    println!("Hello, world!");
}

// ECLI_basic = 0.40 × RentIndex
//            + 0.35 × GroceriesIndex
//            + 0.25 × CostOfLivingIndex

// ECLI_basic = 0.40 × 26.0
//            + 0.35 × 65.1
//            + 0.25 × 66.4

//            = 10.4 + 22.785 + 16.6
//            = 49.785  ≈ 49.8

// Normalized Money = 33,000 / (49.8 / 100)
//                  = 33,000 / 0.498
//                  ≈ 66,265 SEK

// Sarebbe come avere 66,265 SEK a NewYork

// Normalized EUR = (Money in SEK / EUR_exchange_rate) / (ECLI / 100)

// Use this:
// 🔗 European Central Bank (ECB) CPI / HICP
// https://data.ecb.europa.eu/

// Search “HICP inflation euro area” (this is the standard inflation index for Europe)

// Or directly at Eurostat:
// 🔗 https://ec.europa.eu/eurostat

// Search: "HICP EU 27"

// Considering Inflation Too (Take a reference year and check the CPI from that year)

// Real EUR = 50,000 × (100 / 120) / 0.498
//           = 50,000 × 0.833 / 0.498
//           = 41,650 / 0.498
//           = 83,663 EUR real power
