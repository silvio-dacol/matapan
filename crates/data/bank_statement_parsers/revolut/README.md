# How to make a statement

Updated by: Silvio Da Col
Last update: February 2026

1. Access your account on the browser via https://www.revolut.com.
2. Click on your profile icon at the top-right corner.
3. Select **Account Statements**.
4. Select **Excel**.
5. Select **Generate**.

![Image](./pics/1.png)

Note: If you see changes in the statement format, please open an issue or a pull request to update this parser.

## Parser Notes (for contributors)

- This parser currently focuses on bank-account style flows (`accounts` + `transactions`).
- The `instruments` and `positions` parser modules are intentionally present as extension points for users that also use Revolut for investments.
- Pipeline behavior is profile-driven via `utils::PipelineProfile::RetailBankDefault` in the CLI entrypoint.
