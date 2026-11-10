// AUTHR_OS — Shared art-style catalog (v1.6, novelcast parity).
// Pure data mirror of the Rust catalog (`video/style.rs`, §4.18 video_style):
// same ten styles, same prompt fragments — every image prompt (scenes and
// character reference sheets) rides one coherent look per book. The Rust side
// is authoritative for rendering; this copy drives the picker UI and the
// immediate display before `video_style` confirms a switch.

export interface ArtStyle {
  id: string;
  label: string;
  tagline: string;
  /** Prompt fragment injected into image-generation prompts (R2I tier). */
  prompt: string;
  /** Tailwind gradient classes for the picker swatch. */
  swatch: string;
}

export const ART_STYLES: ArtStyle[] = [
  {
    id: 'cinematic',
    label: 'Cinematic Realism',
    tagline: 'Film-still look with dramatic natural light',
    prompt:
      'cinematic film still, photorealistic, shallow depth of field, dramatic natural lighting, rich color grading, ultra detailed',
    swatch: 'from-stone-700 via-amber-900 to-amber-500',
  },
  {
    id: 'storybook',
    label: 'Watercolor Storybook',
    tagline: 'Soft hand-painted warmth, gentle edges',
    prompt:
      'soft watercolor storybook illustration, hand-painted paper texture, gentle pastel palette, warm golden light, delicate ink outlines',
    swatch: 'from-emerald-200 via-teal-100 to-amber-100',
  },
  {
    id: 'manhwa',
    label: 'Modern Manhwa',
    tagline: 'Glossy full-color webtoon, dynamic posing',
    prompt:
      'modern korean manhwa webtoon art, clean digital cel shading, glossy highlights, sharp confident lineart, vibrant saturated colors, dramatic composition',
    swatch: 'from-rose-400 via-orange-300 to-amber-200',
  },
  {
    id: 'anime',
    label: 'Anime Film',
    tagline: 'Hand-painted anime frames, nostalgic glow',
    prompt:
      'hand-painted anime film style, painterly gouache background art, expressive characters, soft nostalgic lighting, detailed clouds and foliage',
    swatch: 'from-teal-300 via-emerald-200 to-yellow-100',
  },
  {
    id: 'manga-ink',
    label: 'Manga Ink',
    tagline: 'Monochrome ink lines, screentone drama',
    prompt:
      'black and white manga ink illustration, bold confident linework, crosshatching and screentone shading, high contrast monochrome',
    swatch: 'from-stone-900 via-stone-600 to-stone-300',
  },
  {
    id: 'oil',
    label: 'Classical Oil Painting',
    tagline: 'Museum-grade brushwork and chiaroscuro',
    prompt:
      'classical oil painting, visible expressive brushstrokes, chiaroscuro lighting, rich earthy pigments, canvas texture, fine-art quality',
    swatch: 'from-amber-800 via-yellow-700 to-stone-400',
  },
  {
    id: 'comic',
    label: 'Western Comic Book',
    tagline: 'Bold inks and halftone print dots',
    prompt:
      'western comic book art, bold ink outlines, halftone dot shading, saturated flat colors, dynamic poses, retro print texture',
    swatch: 'from-red-500 via-orange-400 to-yellow-300',
  },
  {
    id: 'pixar',
    label: '3D Animation',
    tagline: 'Stylized CGI feature-film charm',
    prompt:
      'stylized 3D animated feature film still, soft global illumination, appealing stylized characters, subsurface scattering, vibrant production design',
    swatch: 'from-orange-400 via-amber-300 to-emerald-300',
  },
  {
    id: 'dark-fantasy',
    label: 'Dark Fantasy',
    tagline: 'Moody concept art, epic and ominous',
    prompt:
      'dark fantasy concept art, moody atmospheric fog, epic scale, ornate detail, desaturated palette with ember accents, painterly digital art',
    swatch: 'from-stone-950 via-violet-950 to-rose-900',
  },
  {
    id: 'pulp',
    label: 'Retro Pulp Adventure',
    tagline: 'Vintage paperback-cover energy',
    prompt:
      'vintage pulp adventure paperback cover art, 1950s gouache illustration, dramatic heroic poses, aged paper grain, saturated retro palette',
    swatch: 'from-yellow-600 via-orange-600 to-red-700',
  },
];

export const DEFAULT_ART_STYLE_ID = 'cinematic';

export function getArtStyle(id: string | null | undefined): ArtStyle {
  return ART_STYLES.find((s) => s.id === id) ?? ART_STYLES[0];
}
