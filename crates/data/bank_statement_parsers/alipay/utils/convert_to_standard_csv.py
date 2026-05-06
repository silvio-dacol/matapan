import pandas as pd

path = r"C:\Users\Silvio\git-repos\matapan\crates\data\bank_statement_parsers\alipay\支付宝交易明细(20251201-20260201).csv"

# Find Header Line
# Where we have "交易时间" meaning "Transaction Time" as the first column

with open(path, encoding="gb18030") as f:
    lines = f.readlines()

header_index = None

for i, line in enumerate(lines):
    if line.startswith("交易时间"):
        header_index = i
        break

print("Header at line:", header_index)

df = pd.read_csv(
    path,
    encoding="gb18030",
    skiprows=header_index,
    engine="python"
)

df.to_csv(
    r"C:\Users\Silvio\git-repos\matapan\crates\data\bank_statement_parsers\alipay\output.csv",
    index=False,
    encoding="utf-8-sig"
)

print(df.shape)
print(df.head())
