/**
 * Schema-driven entity panel specs. One generic ResourcePanel renders all
 * six entity domains; `ENUMS` value domains mirror the SQLite CHECK
 * constraints and the Rust DTO enums (snake_case wire values).
 */

import { ENUMS } from './api';

export interface FieldSpec {
  name: string;
  label: string;
  type: 'text' | 'number' | 'select' | 'textarea';
  options?: readonly string[];
  required?: boolean;
  hint?: string;
}

export interface EntitySpec {
  key: string;
  title: string;
  /** Feeds `<select>` for the parent scope of list/create (book or chapter). */
  parent?: { field: string; entity: 'books' | 'chapters'; label: string };
  listCmd: string;
  createCmd: string;
  updateCmd: string;
  deleteCmd: string;
  columns: { name: string; label: string }[];
  createFields: FieldSpec[];
  editFields: FieldSpec[];
}

const EDIT_BASE = (extra: FieldSpec[]): FieldSpec[] => extra;

export const ENTITY_SPECS: EntitySpec[] = [
  {
    key: 'books',
    title: 'Books',
    listCmd: 'book_list',
    createCmd: 'book_create',
    updateCmd: 'book_update',
    deleteCmd: 'book_delete',
    columns: [
      { name: 'id', label: 'ID' },
      { name: 'title', label: 'Title' },
      { name: 'author', label: 'Author' },
      { name: 'narrative_structure', label: 'Structure' },
      { name: 'status', label: 'Status' },
    ],
    createFields: [
      { name: 'title', label: 'Title', type: 'text', required: true },
      { name: 'author', label: 'Author', type: 'text' },
      { name: 'genre', label: 'Genre', type: 'text' },
      { name: 'narrative_structure', label: 'Structure', type: 'select', options: ENUMS.narrative_structure },
      { name: 'status', label: 'Status', type: 'select', options: ENUMS.book_status },
      { name: 'target_word_count', label: 'Target words', type: 'number' },
    ],
    editFields: EDIT_BASE([
      { name: 'title', label: 'Title', type: 'text' },
      { name: 'author', label: 'Author', type: 'text' },
      { name: 'genre', label: 'Genre', type: 'text' },
      { name: 'narrative_structure', label: 'Structure', type: 'select', options: ENUMS.narrative_structure },
      { name: 'status', label: 'Status', type: 'select', options: ENUMS.book_status },
      { name: 'target_word_count', label: 'Target words', type: 'number' },
    ]),
  },
  {
    key: 'chapters',
    title: 'Chapters',
    parent: { field: 'book_id', entity: 'books', label: 'Book' },
    listCmd: 'chapter_list',
    createCmd: 'chapter_create',
    updateCmd: 'chapter_update',
    deleteCmd: 'chapter_delete',
    columns: [
      { name: 'id', label: 'ID' },
      { name: 'chapter_index', label: '#' },
      { name: 'title', label: 'Title' },
      { name: 'status', label: 'Status' },
      { name: 'summary', label: 'Summary' },
    ],
    createFields: [
      { name: 'chapter_index', label: 'Index', type: 'number', hint: 'defaults to next' },
      { name: 'title', label: 'Title', type: 'text' },
      { name: 'summary', label: 'Summary', type: 'textarea' },
    ],
    editFields: EDIT_BASE([
      { name: 'title', label: 'Title', type: 'text' },
      { name: 'summary', label: 'Summary', type: 'textarea' },
      { name: 'status', label: 'Status', type: 'select', options: ENUMS.chapter_status },
    ]),
  },
  {
    key: 'characters',
    title: 'Cast',
    parent: { field: 'book_id', entity: 'books', label: 'Book' },
    listCmd: 'character_list',
    createCmd: 'character_create',
    updateCmd: 'character_update',
    deleteCmd: 'character_delete',
    columns: [
      { name: 'id', label: 'ID' },
      { name: 'character_key', label: 'Key' },
      { name: 'display_name', label: 'Name' },
      { name: 'role', label: 'Role' },
      { name: 'voice_engine', label: 'Voice' },
      { name: 'ipa_phonetic', label: 'IPA' },
    ],
    createFields: [
      { name: 'character_key', label: 'Key (SPEAKER)', type: 'text', required: true },
      { name: 'display_name', label: 'Display name', type: 'text', required: true },
      { name: 'role', label: 'Role', type: 'select', options: ENUMS.character_role },
      { name: 'voice_engine', label: 'Voice engine', type: 'select', options: ENUMS.voice_engine },
      { name: 'ipa_phonetic', label: 'IPA phonetic', type: 'text' },
    ],
    editFields: EDIT_BASE([
      { name: 'character_key', label: 'Key (SPEAKER)', type: 'text' },
      { name: 'display_name', label: 'Display name', type: 'text' },
      { name: 'role', label: 'Role', type: 'select', options: ENUMS.character_role },
      { name: 'voice_engine', label: 'Voice engine', type: 'select', options: ENUMS.voice_engine },
      { name: 'ipa_phonetic', label: 'IPA phonetic', type: 'text' },
    ]),
  },
  {
    key: 'lore',
    title: 'Lorebook',
    parent: { field: 'book_id', entity: 'books', label: 'Book' },
    listCmd: 'lore_list',
    createCmd: 'lore_create',
    updateCmd: 'lore_update',
    deleteCmd: 'lore_delete',
    columns: [
      { name: 'id', label: 'ID' },
      { name: 'term', label: 'Term' },
      { name: 'category', label: 'Category' },
      { name: 'content', label: 'Content' },
    ],
    createFields: [
      { name: 'term', label: 'Term', type: 'text', required: true },
      { name: 'category', label: 'Category', type: 'select', options: ENUMS.lore_category },
      { name: 'content', label: 'Content', type: 'textarea' },
      { name: 'ipa_phonetic', label: 'IPA phonetic', type: 'text' },
    ],
    editFields: EDIT_BASE([
      { name: 'category', label: 'Category', type: 'select', options: ENUMS.lore_category },
      { name: 'content', label: 'Content', type: 'textarea' },
      { name: 'ipa_phonetic', label: 'IPA phonetic', type: 'text' },
    ]),
  },
  {
    key: 'threads',
    title: 'Chekhov Threads',
    parent: { field: 'book_id', entity: 'books', label: 'Book' },
    listCmd: 'thread_list',
    createCmd: 'thread_create',
    updateCmd: 'thread_update',
    deleteCmd: 'thread_delete',
    columns: [
      { name: 'id', label: 'ID' },
      { name: 'title', label: 'Thread' },
      { name: 'status', label: 'Status' },
      { name: 'planted_chapter_index', label: 'Planted' },
      { name: 'payoff_chapter_index', label: 'Payoff' },
    ],
    createFields: [
      { name: 'title', label: 'Title', type: 'text', required: true },
      { name: 'status', label: 'Status', type: 'select', options: ENUMS.thread_status },
      { name: 'planted_chapter_index', label: 'Planted at chapter', type: 'number' },
      { name: 'payoff_chapter_index', label: 'Payoff at chapter', type: 'number' },
      { name: 'notes', label: 'Notes', type: 'textarea' },
    ],
    editFields: EDIT_BASE([
      { name: 'status', label: 'Status', type: 'select', options: ENUMS.thread_status },
      { name: 'payoff_chapter_index', label: 'Payoff at chapter', type: 'number' },
      { name: 'notes', label: 'Notes', type: 'textarea' },
    ]),
  },
  {
    key: 'segments',
    title: 'Segments (AST)',
    parent: { field: 'chapter_id', entity: 'chapters', label: 'Chapter' },
    listCmd: 'segment_list',
    createCmd: 'segment_create',
    updateCmd: 'segment_update',
    deleteCmd: 'segment_delete',
    columns: [
      { name: 'id', label: 'ID' },
      { name: 'segment_id', label: 'Segment' },
      { name: 'speaker_id', label: 'Speaker' },
      { name: 'segment_type', label: 'Type' },
      { name: 'text_content', label: 'Text' },
    ],
    createFields: [
      { name: 'speaker_id', label: 'Speaker key', type: 'text', hint: 'defaults to NARRATOR' },
      { name: 'segment_type', label: 'Type', type: 'select', options: ENUMS.segment_type },
      { name: 'text_content', label: 'Text', type: 'textarea', required: true },
      { name: 'emotion', label: 'Emotion', type: 'text' },
      { name: 'prosody_rate', label: 'Rate', type: 'number' },
      { name: 'prosody_pitch', label: 'Pitch', type: 'number' },
      { name: 'ambient_cue', label: 'Ambient cue', type: 'text' },
      { name: 'sfx_cue', label: 'SFX cue', type: 'text' },
    ],
    editFields: EDIT_BASE([
      { name: 'segment_type', label: 'Type', type: 'select', options: ENUMS.segment_type },
      { name: 'speaker_id', label: 'Speaker key', type: 'text' },
      { name: 'text_content', label: 'Text', type: 'textarea' },
      { name: 'emotion', label: 'Emotion', type: 'text' },
      { name: 'prosody_rate', label: 'Rate', type: 'number' },
      { name: 'prosody_pitch', label: 'Pitch', type: 'number' },
    ]),
  },
];
