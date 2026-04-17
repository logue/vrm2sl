export interface PreviewMotionOption {
  title: string;
  value: string;
  category: PreviewMotionCategory;
  playback: PreviewMotionPlayback;
}

export type PreviewMotionCategory =
  | 'dance'
  | 'locomotion'
  | 'combat'
  | 'social'
  | 'expression'
  | 'sit'
  | 'other';

export interface PreviewMotionCategoryOption {
  title: string;
  value: PreviewMotionCategory | 'system';
}

export type PreviewMotionPlayback = 'loop' | 'oneshot';

const CUSTOM_PREVIEW_MOTION_FILES = [
  'avatar_RPS_countdown.bvh',
  'avatar_RPS_paper.bvh',
  'avatar_RPS_rock.bvh',
  'avatar_RPS_scissors.bvh',
  'avatar_aim_L_bow.bvh',
  'avatar_aim_R_bazooka.bvh',
  'avatar_aim_R_handgun.bvh',
  'avatar_aim_R_rifle.bvh',
  'avatar_angry_fingerwag.bvh',
  'avatar_angry_tantrum.bvh',
  'avatar_away.bvh',
  'avatar_backflip.bvh',
  'avatar_blowkiss.bvh',
  'avatar_bow.bvh',
  'avatar_brush.bvh',
  'avatar_clap.bvh',
  'avatar_courtbow.bvh',
  'avatar_cross_arms.bvh',
  'avatar_crouch.bvh',
  'avatar_crouchwalk.bvh',
  'avatar_curtsy.bvh',
  'avatar_dance1.bvh',
  'avatar_dance2.bvh',
  'avatar_dance3.bvh',
  'avatar_dance4.bvh',
  'avatar_dance5.bvh',
  'avatar_dance6.bvh',
  'avatar_dance7.bvh',
  'avatar_dance8.bvh',
  'avatar_dead.bvh',
  'avatar_drink.bvh',
  'avatar_express_afraid.bvh',
  'avatar_express_anger.bvh',
  'avatar_express_bored.bvh',
  'avatar_express_cry.bvh',
  'avatar_express_embarrased.bvh',
  'avatar_express_laugh.bvh',
  'avatar_express_repulsed.bvh',
  'avatar_express_sad.bvh',
  'avatar_express_shrug.bvh',
  'avatar_express_surprise.bvh',
  'avatar_express_wink.bvh',
  'avatar_express_worry.bvh',
  'avatar_falldown.bvh',
  'avatar_fist_pump.bvh',
  'avatar_fly.bvh',
  'avatar_flyslow.bvh',
  'avatar_hello.bvh',
  'avatar_hold_L_bow.bvh',
  'avatar_hold_R_bazooka.bvh',
  'avatar_hold_R_handgun.bvh',
  'avatar_hold_R_rifle.bvh',
  'avatar_hold_throw_R.bvh',
  'avatar_hover.bvh',
  'avatar_hover_down.bvh',
  'avatar_hover_up.bvh',
  'avatar_impatient.bvh',
  'avatar_jump.bvh',
  'avatar_jumpforjoy.bvh',
  'avatar_kick_roundhouse_R.bvh',
  'avatar_kissmybutt.bvh',
  'avatar_kneel_left.bvh',
  'avatar_kneel_right.bvh',
  'avatar_land.bvh',
  'avatar_laugh_short.bvh',
  'avatar_motorcycle_sit.bvh',
  'avatar_musclebeach.bvh',
  'avatar_no_head.bvh',
  'avatar_no_unhappy.bvh',
  'avatar_nyanya.bvh',
  'avatar_peace.bvh',
  'avatar_point_me.bvh',
  'avatar_point_you.bvh',
  'avatar_prejump.bvh',
  'avatar_punch_L.bvh',
  'avatar_punch_R.bvh',
  'avatar_punch_onetwo.bvh',
  'avatar_run.bvh',
  'avatar_salute.bvh',
  'avatar_shoot_L_bow.bvh',
  'avatar_shout.bvh',
  'avatar_sit.bvh',
  'avatar_sit_female.bvh',
  'avatar_sit_generic.bvh',
  'avatar_sit_ground.bvh',
  'avatar_sit_ground_constrained.bvh',
  'avatar_sit_to_stand.bvh',
  'avatar_sleep.bvh',
  'avatar_slowwalk.bvh',
  'avatar_smoke_idle.bvh',
  'avatar_smoke_inhale.bvh',
  'avatar_smoke_throw_down.bvh',
  'avatar_snapshot.bvh',
  'avatar_soft_land.bvh',
  'avatar_stand.bvh',
  'avatar_stand_2.bvh',
  'avatar_stand_3.bvh',
  'avatar_stand_4.bvh',
  'avatar_standup.bvh',
  'avatar_stretch.bvh',
  'avatar_stride.bvh',
  'avatar_surf.bvh',
  'avatar_sword_strike_R.bvh',
  'avatar_talk.bvh',
  'avatar_throw_R.bvh',
  'avatar_tryon_shirt.bvh',
  'avatar_turn_180.bvh',
  'avatar_turnback_180.bvh',
  'avatar_turnleft.bvh',
  'avatar_turnright.bvh',
  'avatar_type.bvh',
  'avatar_uphillwalk.bvh',
  'avatar_whisper.bvh',
  'avatar_whistle.bvh',
  'avatar_wink_hollywood.bvh',
  'avatar_yes_happy.bvh',
  'avatar_yes_head.bvh',
  'avatar_yoga_float.bvh'
] as const;

export function formatPreviewMotionTitle(source: string): string {
  const filename = source.split('/').pop() ?? source;
  const baseName = filename.replace(/^avatar_/, '').replace(/\.bvh$/i, '');

  return baseName
    .split('_')
    .filter(Boolean)
    .map(token => {
      if (token === token.toUpperCase() && /[A-Z]/.test(token)) {
        return token;
      }
      return token.charAt(0).toUpperCase() + token.slice(1);
    })
    .join(' ');
}

function categorizePreviewMotion(file: string): PreviewMotionCategory {
  const name = file.toLowerCase();

  if (/dance|nyanya|yoga/.test(name)) {
    return 'dance';
  }

  if (/walk|run|stride|turn|jump|land|fly|hover|crouchwalk|uphillwalk|prejump/.test(name)) {
    return 'locomotion';
  }

  if (/punch|kick|sword|shoot|aim_|hold_|throw|bazooka|handgun|rifle|\bbow\b/.test(name)) {
    return 'combat';
  }

  if (/sit|kneel|sleep|motorcycle|crouch/.test(name)) {
    return 'sit';
  }

  if (
    /express_|angry|laugh|cry|worry|bored|surprise|embarrased|repulsed|afraid|shout|impatient|kiss|whisper|whistle|smoke|yes_|no_/.test(
      name
    )
  ) {
    return 'expression';
  }

  if (
    /clap|courtbow|curtsy|hello|salute|peace|point_|rps_|talk|type|brush|drink|snapshot|stretch/.test(
      name
    )
  ) {
    return 'social';
  }

  return 'other';
}

export function detectPreviewMotionPlayback(file: string): PreviewMotionPlayback {
  const name = file.toLowerCase();
  const oneShotKeywords = [
    'bow',
    'courtbow',
    'curtsy',
    'clap',
    'hello',
    'salute',
    'point_',
    'snapshot',
    'drink',
    'brush',
    'stretch',
    'laugh_short',
    'jumpforjoy',
    'prejump',
    'jump',
    'land',
    'soft_land',
    'falldown',
    'standup',
    'sit_to_stand',
    'kick',
    'punch',
    'sword',
    'shoot',
    'throw',
    'blowkiss',
    'fist_pump',
    'shout',
    'wink',
    'peace',
    'turn_180',
    'turnback_180'
  ];

  if (oneShotKeywords.some(keyword => name.includes(keyword))) {
    return 'oneshot';
  }

  return 'loop';
}

export function playbackIcon(playback: PreviewMotionPlayback): string {
  if (playback === 'loop') {
    return 'mdi-repeat';
  }
  return 'mdi-play-circle-outline';
}

export const PREVIEW_MOTION_CATEGORY_OPTIONS: PreviewMotionCategoryOption[] = [
  { title: 'System', value: 'system' },
  { title: 'Locomotion', value: 'locomotion' },
  { title: 'Dance', value: 'dance' },
  { title: 'Combat', value: 'combat' },
  { title: 'Social', value: 'social' },
  { title: 'Expression', value: 'expression' },
  { title: 'Sit / Ground', value: 'sit' },
  { title: 'Other', value: 'other' }
];

export const PREVIEW_CUSTOM_MOTION_OPTIONS: PreviewMotionOption[] = CUSTOM_PREVIEW_MOTION_FILES.map(
  file => ({
    title: formatPreviewMotionTitle(file),
    value: `/animations/${file}`,
    category: categorizePreviewMotion(file),
    playback: detectPreviewMotionPlayback(file)
  })
);
