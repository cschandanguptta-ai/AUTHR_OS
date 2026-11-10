export type PanelKey =
  | 'books' | 'chapters' | 'characters' | 'lore' | 'threads' | 'segments'
  | 'search' | 'security' | 'git';

const NAV: { key: PanelKey; label: string; hint: string }[] = [
  { key: 'books', label: 'Books', hint: 'book_list / create' },
  { key: 'chapters', label: 'Chapters', hint: 'chapter_list / create' },
  { key: 'characters', label: 'Cast', hint: 'character_list / create' },
  { key: 'lore', label: 'Lorebook', hint: 'lore_list / create' },
  { key: 'threads', label: 'Threads', hint: 'thread_list / create' },
  { key: 'segments', label: 'Segments', hint: 'segment_list / create' },
  { key: 'search', label: 'Search', hint: 'search_segments (FTS5)' },
  { key: 'security', label: 'Security', hint: 'sandbox_resolve / info' },
  { key: 'git', label: 'Git', hint: 'project_git_log' },
];

interface Props {
  active: PanelKey;
  onSelect: (key: PanelKey) => void;
  enabled: boolean;
}

export function Sidebar({ active, onSelect, enabled }: Props) {
  return (
    <nav className={`sidebar ${enabled ? '' : 'disabled'}`}>
      <div className="sidebar-group">Manuscript</div>
      {NAV.slice(0, 6).map((n) => (
        <button
          key={n.key}
          className={`nav-item ${active === n.key ? 'active' : ''}`}
          onClick={() => onSelect(n.key)}
          title={n.hint}
        >
          {n.label}
        </button>
      ))}
      <div className="sidebar-group">Introspection</div>
      {NAV.slice(6).map((n) => (
        <button
          key={n.key}
          className={`nav-item ${active === n.key ? 'active' : ''}`}
          onClick={() => onSelect(n.key)}
          title={n.hint}
        >
          {n.label}
        </button>
      ))}
    </nav>
  );
}
