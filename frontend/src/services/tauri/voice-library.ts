import type {
  SyncVoicesRequest,
  VoiceDetail,
  VoiceMutationResult,
  VoiceSummary,
  VoiceSyncResult,
} from '../../utils/types/voice';
import { invokeWithMockFallback } from './invoke';

const mockVoiceDetails: VoiceDetail[] = [
  {
    voiceName: 'gentle_female',
    displayName: '温柔女声',
    source: 'preset',
    tags: ['预置', '普通话', '柔和'],
    hasReferenceAudio: true,
    updatedAt: '2026-05-06 20:15',
    referenceTextPreview: '你好，我会用更自然的语气完成这段声音转换。',
    syncStatus: 'synced',
    isCurrent: true,
    voiceInstruction: '保持温柔、清晰、轻微贴近麦克风的亲密感。',
    referenceText: '你好，我会用更自然的语气完成这段声音转换，让输出保持稳定、柔和并且清晰。',
    referenceAudioPath: '~/voice-cloner/cache/preset-preview/gentle_female.wav',
    previewAudioPath: '~/voice-cloner/cache/preset-preview/gentle_female_preview.wav',
    editable: false,
  },
  {
    voiceName: 'teen_boy',
    displayName: '少年音',
    source: 'preset',
    tags: ['预置', '轻快'],
    hasReferenceAudio: true,
    updatedAt: '2026-05-05 18:02',
    referenceTextPreview: '这是一段偏年轻、明亮的参考文本。',
    syncStatus: 'synced',
    isCurrent: false,
    voiceInstruction: '语速略快，保留明亮干净的少年感。',
    referenceText: '这是一段偏年轻、明亮的参考文本，用来测试实时变声时的稳定性。',
    referenceAudioPath: '~/voice-cloner/cache/preset-preview/teen_boy.wav',
    previewAudioPath: '~/voice-cloner/cache/preset-preview/teen_boy_preview.wav',
    editable: false,
  },
  {
    voiceName: 'radio_male',
    displayName: '电台男声',
    source: 'custom',
    tags: ['自定义', '低沉', '播客'],
    hasReferenceAudio: true,
    updatedAt: '2026-05-07 09:40',
    referenceTextPreview: '欢迎收听今晚的声音实验室，我们从一段低沉旁白开始。',
    syncStatus: 'remoteChanged',
    isCurrent: false,
    voiceInstruction: '低频更扎实，适合播客开场和旁白。',
    referenceText: '欢迎收听今晚的声音实验室，我们从一段低沉旁白开始，保留电台质感和清晰的尾音。',
    referenceAudioPath: '~/voice-cloner/library/custom-voices/radio_male.wav',
    previewAudioPath: '~/voice-cloner/cache/preset-preview/radio_male_preview.wav',
    editable: true,
  },
  {
    voiceName: 'mechanic',
    displayName: '机械音',
    source: 'custom',
    tags: ['自定义', '实验'],
    hasReferenceAudio: false,
    updatedAt: '2026-05-04 11:28',
    referenceTextPreview: '缺少参考音频，保存前需要重新上传样本。',
    syncStatus: 'failed',
    isCurrent: false,
    voiceInstruction: '带一点金属感，但不要破坏语义清晰度。',
    referenceText: '缺少参考音频，保存前需要重新上传样本。',
    editable: true,
  },
];

function toSummary(detail: VoiceDetail): VoiceSummary {
  return {
    voiceName: detail.voiceName,
    displayName: detail.displayName,
    source: detail.source,
    tags: detail.tags,
    hasReferenceAudio: detail.hasReferenceAudio,
    updatedAt: detail.updatedAt,
    referenceTextPreview: detail.referenceTextPreview,
    syncStatus: detail.syncStatus,
    isCurrent: detail.isCurrent,
  };
}

function firstMockVoice(): VoiceDetail {
  const [firstVoice] = mockVoiceDetails;

  if (!firstVoice) {
    throw new Error('Voice mock data must include at least one voice.');
  }

  return firstVoice;
}

export async function listVoices(): Promise<VoiceSummary[]> {
  return invokeWithMockFallback('list_voices', () => mockVoiceDetails.map(toSummary));
}

export async function getVoiceDetail(voiceName: string): Promise<VoiceDetail> {
  return invokeWithMockFallback(
    'get_voice_detail',
    () => mockVoiceDetails.find((voice) => voice.voiceName === voiceName) ?? firstMockVoice(),
    { voiceName }
  );
}

export async function saveVoiceDetail(detail: VoiceDetail): Promise<VoiceMutationResult> {
  return invokeWithMockFallback(
    'update_voice',
    () => ({
      voiceName: detail.voiceName,
      message: `${detail.displayName} 已保存到本地草稿`,
      updatedAt: new Date().toLocaleString('zh-CN', { hour12: false }),
    }),
    { input: detail }
  );
}

export async function syncVoices(request: SyncVoicesRequest): Promise<VoiceSyncResult> {
  return invokeWithMockFallback(
    'sync_voices',
    () => ({
      mode: request.mode,
      syncedCount: request.voiceNames?.length ?? mockVoiceDetails.length - 1,
      failedCount: request.mode === 'retryFailed' ? 0 : 1,
      message: request.mode === 'full' ? '已完成全量同步模拟' : '已完成增量同步模拟',
    }),
    { request }
  );
}
