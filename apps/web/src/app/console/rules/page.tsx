export default function RulesPage() {
  return (
    <section className="rounded-xl border border-[var(--line)] bg-[var(--surface)] p-5">
      <h2 className="text-xl font-semibold">Rules</h2>
      <p className="mt-2 text-sm text-[var(--ink-2)]">
        Rule editing will use a preview-before-apply workflow for safer operations.
      </p>

      <div className="mt-4 rounded-lg border border-[var(--line)] bg-[var(--brand-soft)] p-4 text-sm">
        <p className="font-semibold">Next implementation step</p>
        <p className="mt-1 text-[var(--ink-2)]">
          Add rule JSON editor + dry-run endpoint integration to show impacted
          transaction count before write.
        </p>
      </div>
    </section>
  );
}
