import { useCallback, useEffect, useState } from 'react';
import type { Envelope } from '../api';
import type { EntitySpec, FieldSpec } from '../schema';
import type { Transport } from '../transport';

interface Props {
  spec: EntitySpec;
  transport: Transport;
  mounted: boolean;
  /** id of the selected parent (book/chapter) chosen in the header. */
  parentId: number | null;
  parentLabel: string;
  /** rows of the parent entity for the selector. */
  parents: { id: number; label: string }[];
  onParentChange: (id: number | null) => void;
  /** id of the selected chapter when segments pick their book indirectly. */
  parentSecondary?: { id: number | null; rows: { id: number; label: string }[]; onChange: (id: number | null) => void } | null;
  onEnvelope: (env: Envelope, cmd: string) => void;
  onMutation: () => void;
}

function emptyForm(fields: FieldSpec[]): Record<string, any> {
  const out: Record<string, any> = {};
  for (const f of fields) out[f.name] = '';
  return out;
}

/** Generic schema-driven CRUD panel for one entity domain. */
export function ResourcePanel(props: Props) {
  const { spec, transport, mounted, parentId, onEnvelope, onMutation } = props;
  const [rows, setRows] = useState<any[]>([]);
  const [form, setForm] = useState<Record<string, any>>(() => emptyForm(spec.createFields));
  const [editing, setEditing] = useState<any | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!mounted) return;
    const payload: Record<string, any> = {};
    if (spec.parent && parentId !== null) payload[spec.parent.field] = parentId;
    const res = await transport.invoke(spec.listCmd, payload);
    onEnvelope(res, spec.listCmd);
    if (res.ok) setRows(res.data);
  }, [transport, spec, mounted, parentId, onEnvelope]);

  useEffect(() => { void load(); }, [load]);

  const submit = async () => {
    const payload: Record<string, any> = {};
    if (spec.parent && parentId !== null) payload[spec.parent.field] = parentId;
    for (const f of spec.createFields) {
      const v = form[f.name];
      if (v === '' || v === undefined) continue;
      payload[f.name] = f.type === 'number' ? Number(v) : v;
    }
    const res = editing
      ? await transport.invoke(spec.updateCmd, { id: editing.id, ...payload })
      : await transport.invoke(spec.createCmd, payload);
    onEnvelope(res, editing ? spec.updateCmd : spec.createCmd);
    if (res.ok) {
      setNotice(editing ? `${spec.title}: #${res.data.id} updated` : `${spec.title}: #${res.data.id} created`);
      setForm(emptyForm(spec.createFields));
      setEditing(null);
      onMutation();
    } else {
      setNotice(`[${res.error.code}] ${res.error.message}`);
    }
  };

  const remove = async (row: any) => {
    const res = await transport.invoke(spec.deleteCmd, { id: row.id });
    onEnvelope(res, spec.deleteCmd);
    if (res.ok) {
      setNotice(`${spec.title}: #${row.id} deleted`);
      if (editing?.id === row.id) setEditing(null);
      onMutation();
    } else {
      setNotice(`[${res.error.code}] ${res.error.message}`);
    }
  };

  const edit = (row: any) => {
    setEditing(row);
    const next: Record<string, any> = {};
    for (const f of spec.editFields) next[f.name] = row[f.name] ?? '';
    setForm(next);
    setNotice(null);
  };

  const renderField = (f: FieldSpec) => (
    <label key={f.name} className="field">
      <span>{f.label}{f.required ? ' *' : ''}</span>
      {f.type === 'select' ? (
        <select
          value={form[f.name] ?? ''}
          onChange={(e) => setForm({ ...form, [f.name]: e.target.value })}
        >
          <option value="">—</option>
          {(f.options ?? []).map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      ) : f.type === 'textarea' ? (
        <textarea
          rows={2}
          value={form[f.name] ?? ''}
          onChange={(e) => setForm({ ...form, [f.name]: e.target.value })}
        />
      ) : (
        <input
          type={f.type === 'number' ? 'number' : 'text'}
          step="any"
          value={form[f.name] ?? ''}
          onChange={(e) => setForm({ ...form, [f.name]: e.target.value })}
        />
      )}
      {f.hint && <em>{f.hint}</em>}
    </label>
  );

  return (
    <section className="panel">
      <div className="panel-head">
        <h2>{spec.title}</h2>
        {spec.parent && (
          <label className="parent-select">
            <span>{props.parentLabel}</span>
            <select
              value={parentId ?? ''}
              onChange={(e) => props.onParentChange(e.target.value ? Number(e.target.value) : null)}
            >
              <option value="">— select —</option>
              {props.parents.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
            </select>
          </label>
        )}
        {props.parentSecondary && (
          <label className="parent-select">
            <span>Book</span>
            <select
              value={props.parentSecondary.id ?? ''}
              onChange={(e) => props.parentSecondary!.onChange(e.target.value ? Number(e.target.value) : null)}
            >
              <option value="">— select —</option>
              {props.parentSecondary.rows.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
            </select>
          </label>
        )}
      </div>

      {!mounted && <div className="empty">Mount a bundle to load {spec.title.toLowerCase()}.</div>}

      {mounted && (
        <>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>{spec.columns.map((c) => <th key={c.name}>{c.label}</th>)}<th /></tr>
              </thead>
              <tbody>
                {rows.length === 0 && (
                  <tr><td colSpan={spec.columns.length + 1} className="empty">No rows.</td></tr>
                )}
                {rows.map((row) => (
                  <tr key={row.id} className={editing?.id === row.id ? 'selected' : ''}>
                    {spec.columns.map((c) => (
                      <td key={c.name} title={String(row[c.name] ?? '')}>
                        {row[c.name] === null || row[c.name] === undefined ? '' : String(row[c.name])}
                      </td>
                    ))}
                    <td className="row-actions">
                      <button className="btn small" onClick={() => edit(row)}>edit</button>
                      <button className="btn small danger" onClick={() => remove(row)}>del</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <div className="form-card">
            <h3>{editing ? `Edit #${editing.id} (${spec.updateCmd})` : `Create (${spec.createCmd})`}</h3>
            <div className="form-grid">
              {spec.parent && !editing && (
                <label className="field">
                  <span>{props.parentLabel} id</span>
                  <input value={parentId ?? ''} disabled />
                </label>
              )}
              {(editing ? spec.editFields : spec.createFields).map(renderField)}
            </div>
            <div className="form-actions">
              <button className="btn primary" onClick={submit} disabled={!mounted}>
                {editing ? 'Save update' : 'Create'}
              </button>
              {editing && (
                <button className="btn" onClick={() => { setEditing(null); setForm(emptyForm(spec.createFields)); }}>
                  Cancel
                </button>
              )}
            </div>
          </div>

          {notice && <div className="notice">{notice}</div>}
        </>
      )}
    </section>
  );
}
