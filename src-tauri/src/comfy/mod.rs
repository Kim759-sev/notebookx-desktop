use crate::types::*;
use regex::Regex;
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::LazyLock,
    time::Duration,
};

pub const BOOTSTRAP_COMFY_PY: &str =
    include_str!("../../../plugins/comfyui-studio/bootstrap_comfy.py");
pub const LLAMA_WORKER_PY: &str = include_str!("../../../plugins/llama-cpp/worker.py");
const DRIVE_ROOT: &str = "/content/drive/MyDrive/DramaSmith-H3";
const MODELS: &[(&str, ModelCategory)] = &[
    (
        "minimax_h3_fl2va_pruned_bf16.safetensors",
        ModelCategory::DiffusionModels,
    ),
    (
        "qwen3vl_32b_minimax_h3_bf16.safetensors",
        ModelCategory::TextEncoders,
    ),
    ("minimax_h3_video_vae_fp16.safetensors", ModelCategory::Vae),
    ("minimax_h3_audio_vae_fp32.safetensors", ModelCategory::Vae),
    (
        "minimax_h3_fl2v_turbo_4step_v1.2_768p_comfyui_bf16.safetensors",
        ModelCategory::Loras,
    ),
    ("电影感loraV1.0正式版.safetensors", ModelCategory::Loras),
];

pub fn default_drive_models_seed() -> Vec<DriveModelItem> {
    MODELS
        .iter()
        .enumerate()
        .map(|(i, &(name, category))| DriveModelItem {
            filename: name.into(),
            relativePath: format!(
                "{}/{name}",
                if i == 5 {
                    "loras"
                } else {
                    category.drive_subdir()
                }
            ),
            category,
            sizeBytes: 0,
            updatedAt: None,
        })
        .collect()
}

pub fn default_workflow_profiles() -> Vec<WorkflowProfile> {
    let profile =
        |id: &str, name: &str, description: &str, icon: &str, accelerator, min_vram, idle| {
            WorkflowProfile {
                id: id.into(),
                name: name.into(),
                description: description.into(),
                icon: icon.into(),
                accelerator,
                highMem: false,
                minVramGb: min_vram,
                maxRetries: 3,
                driveMountMode: DriveMountMode::ColabNative,
                driveRootPath: DRIVE_ROOT.into(),
                startupMode: ProfileStartupMode::ComfySnapshot,
                managedScriptRelPath: None,
                comfyPort: 8188,
                managementPort: None,
                requiredNodes: vec![],
                syncDramaSmith: false,
                costGuard: CostGuardPolicy {
                    idleStopMinutes: Some(idle),
                    ..Default::default()
                },
                defaultWorkflowJsonPath: None,
            }
        };
    vec![
        WorkflowProfile {
            startupMode: ProfileStartupMode::ManagedScript,
            managedScriptRelPath: Some("workflows/startup-managed-v2.py".into()),
            managementPort: Some(8189),
            requiredNodes: vec!["H3C_Timeline".into(), "H3C_SaveSegment".into()],
            syncDramaSmith: true,
            defaultWorkflowJsonPath: Some("workflows/H3-WenWu-BF16-Continuity.json".into()),
            ..profile(
                "dramasmith-h3-bf16",
                "DramaSmith 文武 H3 · BF16 连续性出片 (G4 96GB)",
                "恢复云盘上的文武单采环境，载入首帧或沿用上一段的连续性缓存。",
                "🎬",
                AcceleratorKind::G4,
                90,
                30,
            )
        },
        profile(
            "flux-sdxl-image",
            "Flux / SDXL 高清生图与精修工作流 (L4 24GB)",
            "复用 Drive 模型库及已保存的工作流，生成、精修和放大图片。",
            "🎨",
            AcceleratorKind::L4,
            20,
            20,
        ),
        WorkflowProfile {
            highMem: true,
            maxRetries: 4,
            ..profile(
                "wan-hunyuan-video",
                "Wan2.1 / Hunyuan 通用图生视频 (A100 80GB)",
                "申请高内存 A100，用本地 SSD 运行视频工作流并将输出保存至云盘。",
                "🎞️",
                AcceleratorKind::A100,
                75,
                30,
            )
        },
        WorkflowProfile {
            startupMode: ProfileStartupMode::CpuPreDownload,
            costGuard: CostGuardPolicy {
                autoStopMinutes: Some(90),
                idleStopMinutes: Some(15),
                ..Default::default()
            },
            ..profile(
                "cpu-model-preloader",
                "CPU 零显卡积分 · 提前下载模型与配环境",
                "使用 CPU 将所缺模型直接下载到 Drive，并更新模型清单。",
                "📥",
                AcceleratorKind::Cpu,
                0,
                15,
            )
        },
    ]
}

fn binding(
    key: &str,
    label: &str,
    node: &str,
    field: &str,
    kind: &str,
    default: Value,
) -> WorkflowParamBinding {
    WorkflowParamBinding {
        key: key.into(),
        label: label.into(),
        nodeId: node.into(),
        inputField: field.into(),
        valueType: kind.into(),
        defaultValue: default,
    }
}

pub fn default_saved_api_workflows() -> Vec<SavedApiWorkflow> {
    let prompt = "A continuous cinematic shot, natural light";
    let timeline = json!({"version":1,"prompt":"","boxes":[{"id":"shot01","frames":243,"prompt":prompt,"seed":42,"cut":false,"first":"image1","last":"","memory":"scene"}]}).to_string();
    // Active branch of H3-WenWu-BF16-Continuity, for H3 Continuity Studio 2f2bcf6.
    // The optional semantic bridge and zero-strength LoRAs do not affect this branch.
    let h3 = json!({
        "1":{"class_type":"VAELoader","inputs":{"vae_name":MODELS[2].0}},
        "2":{"class_type":"VAELoader","inputs":{"vae_name":MODELS[3].0}},
        "3":{"class_type":"CLIPLoader","inputs":{"clip_name":MODELS[1].0,"type":"minimax","device":"default"}},
        "4":{"class_type":"UNETLoader","inputs":{"unet_name":MODELS[0].0,"weight_dtype":"default"}},
        "5":{"class_type":"LoraLoaderBypassModelOnly","inputs":{"model":["4",0],"lora_name":MODELS[4].0,"strength_model":1.0}},
        "6":{"class_type":"LoadImage","inputs":{"image":"first-frame.png"}},
        "8":{"class_type":"MiniMaxH3DualClockSamplerT8","inputs":{"model":["48",0],"av_latent":["100",2],"steps":8,"shift_video":12,"shift_audio":3,"sampler_name":"dual_clock_euler","scheduler":"native_flow"}},
        "10":{"class_type":"BasicGuider","inputs":{"model":["8",0],"conditioning":["100",0]}},
        "11":{"class_type":"RandomNoise","inputs":{"noise_seed":["100",4]}},
        "12":{"class_type":"SamplerCustomAdvanced","inputs":{"noise":["11",0],"guider":["10",0],"sampler":["8",1],"sigmas":["8",2],"latent_image":["100",2]}},
        "20":{"class_type":"MiniMaxH3AVDecodeT8","inputs":{"av_latent":["12",0],"video_vae":["1",0],"audio_vae":["2",0]}},
        "33":{"class_type":"ModelAttentionBackend","inputs":{"model":["5",0],"attention":"pytorch attention"}},
        "48":{"class_type":"LoraLoaderModelOnly","inputs":{"model":["33",0],"lora_name":MODELS[5].0,"strength_model":0.2}},
        "100":{"class_type":"H3C_Timeline","inputs":{"clip":["3",0],"vae":["1",0],"audio_vae":["2",0],"images.image1":["6",0],"name":"wenwu_bf16_continuity","width":1056,"height":608,"context_frames":"39","carry_audio":true,"ref_image_size":"match","audio_context_frames":0,"timeline":timeline}},
        "101":{"class_type":"H3C_SaveSegment","inputs":{"segment":["100",3],"latent":["12",0],"images":["20",0],"audio":["20",1],"description":["100",5],"fps":24,"write_full_video":true,"crf":22,"color_space":"sRGB","codec":"HEVC"}}
    });
    let checkpoint = "sd_xl_base_1.0.safetensors";
    let image = json!({
        "3":{"class_type":"KSampler","inputs":{"model":["4",0],"positive":["6",0],"negative":["7",0],"latent_image":["5",0],"seed":42,"steps":25,"cfg":6.5,"sampler_name":"euler","scheduler":"normal","denoise":1.0}},
        "4":{"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name":checkpoint}},
        "5":{"class_type":"EmptyLatentImage","inputs":{"width":1024,"height":1024,"batch_size":1}},
        "6":{"class_type":"CLIPTextEncode","inputs":{"clip":["4",1],"text":prompt}},
        "7":{"class_type":"CLIPTextEncode","inputs":{"clip":["4",1],"text":""}},
        "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["4",2]}},
        "9":{"class_type":"SaveImage","inputs":{"images":["8",0],"filename_prefix":"NotebookX"}}
    });
    let updated = "2026-11-20T00:00:00Z";
    vec![
        SavedApiWorkflow {
            id: "colab-bf16-continuity".into(),
            name: "文武 H3 · BF16 连续性".into(),
            description: "以首帧开拍，按同一序列的上一个任务续接。".into(),
            profileId: "dramasmith-h3-bf16".into(),
            outputKind: "video".into(),
            supportsContinuity: true,
            bindings: vec![
                binding(
                    "prompt",
                    "提示词",
                    "100",
                    "timeline.boxes.0.prompt",
                    "string",
                    json!(prompt),
                ),
                binding(
                    "seed",
                    "随机种子",
                    "100",
                    "timeline.boxes.0.seed",
                    "number",
                    json!(42),
                ),
                binding(
                    "frames",
                    "帧数",
                    "100",
                    "timeline.boxes.0.frames",
                    "number",
                    json!(243),
                ),
                binding(
                    "image",
                    "首帧图片",
                    "6",
                    "image",
                    "string",
                    json!("first-frame.png"),
                ),
            ],
            apiGraphJson: h3,
            updatedAt: updated.into(),
        },
        SavedApiWorkflow {
            id: "universal-txt2img".into(),
            name: "通用 SDXL 文生图".into(),
            description: "使用标准 ComfyUI 节点采样、解码和保存图片。".into(),
            profileId: "flux-sdxl-image".into(),
            outputKind: "image".into(),
            supportsContinuity: false,
            bindings: vec![
                binding("prompt", "提示词", "6", "text", "string", json!(prompt)),
                binding("negative", "负面提示词", "7", "text", "string", json!("")),
                binding("seed", "随机种子", "3", "seed", "number", json!(42)),
                binding(
                    "checkpoint",
                    "主模型",
                    "4",
                    "ckpt_name",
                    "string",
                    json!(checkpoint),
                ),
            ],
            apiGraphJson: image,
            updatedAt: updated.into(),
        },
    ]
}

pub fn apply_workflow_params(
    workflow: &SavedApiWorkflow,
    params: &HashMap<String, Value>,
) -> Result<Value, String> {
    let mut graph = workflow.apiGraphJson.clone();
    for binding in &workflow.bindings {
        let Some(value) = params.get(&binding.key) else {
            continue;
        };
        let inputs = graph
            .get_mut(&binding.nodeId)
            .and_then(|n| n.get_mut("inputs"))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("Missing inputs on node {}", binding.nodeId))?;
        if let Some(key) = binding.inputField.strip_prefix("timeline.boxes.0.") {
            let mut timeline: Value = serde_json::from_str(
                inputs
                    .get("timeline")
                    .and_then(Value::as_str)
                    .ok_or("Invalid H3 timeline")?,
            )
            .map_err(|e| e.to_string())?;
            *timeline
                .pointer_mut(&format!("/boxes/0/{key}"))
                .ok_or("Invalid H3 shot binding")? = value.clone();
            inputs.insert("timeline".into(), json!(timeline.to_string()));
        } else {
            inputs.insert(binding.inputField.clone(), value.clone());
        }
    }
    Ok(graph)
}

/// The caller resolves parentTaskId to a completed graph on the same Drive root.
pub fn configure_h3_sequence(
    graph: &mut Value,
    sequence: &str,
    parent: Option<&Value>,
) -> Result<(), String> {
    if sequence.trim().is_empty()
        || sequence == "."
        || sequence == ".."
        || sequence.contains(['/', '\\'])
    {
        return Err("Invalid H3 sequence name".into());
    }
    let inputs = graph
        .pointer_mut("/100/inputs")
        .filter(|v| v.is_object())
        .ok_or("Missing H3 timeline node")?;
    inputs["name"] = json!(sequence);
    if let Some(parent) = parent {
        let previous = parent
            .pointer("/100/inputs")
            .ok_or("Parent has no H3 timeline")?;
        if previous["name"].as_str() != Some(sequence) {
            return Err("Parent belongs to a different H3 sequence".into());
        }
        let mut timeline: Value = serde_json::from_str(
            previous["timeline"]
                .as_str()
                .ok_or("Missing parent timeline")?,
        )
        .map_err(|e| e.to_string())?;
        let next: Value = serde_json::from_str(
            inputs["timeline"]
                .as_str()
                .ok_or("Missing child timeline")?,
        )
        .map_err(|e| e.to_string())?;
        let mut shot = next
            .pointer("/boxes/0")
            .filter(|v| v.is_object())
            .ok_or("Missing child shot")?
            .clone();
        let boxes = timeline
            .get_mut("boxes")
            .and_then(Value::as_array_mut)
            .filter(|b| !b.is_empty())
            .ok_or("Missing parent shots")?;
        shot["id"] = json!(format!("shot{:02}", boxes.len() + 1));
        shot["first"] = json!("");
        boxes.push(shot);
        inputs["timeline"] = json!(timeline.to_string());
        // Keep the original first-frame input for existing boxes.
        if let Some(image) = parent.pointer("/6/inputs/image") {
            *graph
                .pointer_mut("/6/inputs/image")
                .ok_or("Missing first-frame input")? = image.clone();
        }
    }
    Ok(())
}

fn category(node: &str) -> ModelCategory {
    let node = node.to_ascii_lowercase();
    if node.contains("bridge") {
        ModelCategory::SemanticBridge
    } else if node.contains("lora") {
        ModelCategory::Loras
    } else if node.contains("vae") {
        ModelCategory::Vae
    } else if node.contains("clipvision") {
        ModelCategory::ClipVision
    } else if ["cliploader", "textencode", "t5", "qwen"]
        .iter()
        .any(|s| node.contains(s))
    {
        ModelCategory::TextEncoders
    } else if node.contains("controlnet") {
        ModelCategory::Controlnet
    } else if node.contains("upscale") {
        ModelCategory::UpscaleModels
    } else if ["unet", "diffusion", "h3"].iter().any(|s| node.contains(s)) {
        ModelCategory::DiffusionModels
    } else {
        ModelCategory::Checkpoints
    }
}

// API exports omit registry IDs. Cache the ComfyUI-Manager core mapping for
// offline scans, plus legacy and frontend-only nodes. UI registry IDs override it.
// Registry revision 2908ccdd6f567e06040ed42c96f205c09f27b7c8 (extension-node-map.json).
static CORE_NODES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    r#"APG|ARVideoI2V|AddLayer|AddNoise|AlignYourStepsScheduler|AnimaLLLiteApply|ApplyTextureToMesh
AudioAdjustVolume|AudioConcat|AudioEncoderEncode|AudioEncoderLoader|AudioEqualizer3Band|AudioMerge
AutogrowNamesTestNode|AutogrowPrefixTestNode|BakeAmbientOcclusion|BakeNormalMapFromMesh
BakeTextureFromViews|BakeTextureFromVoxel|BasicGuider|BasicScheduler|BatchCameras
BatchImagesMasksLatentsNode|BatchImagesNode|BatchLatentsNode|BatchMasksNode|BeebleSwitchXImageEdit
BeebleSwitchXVideoEdit|BerniniConditioning|BetaSamplingScheduler|BlockSparseAttention|BranchNode
BreezeTextToSpeech|BreezeVoiceClone|BreezeVoiceDesign|BreezeVoiceLocalize|BreezeVoiceRemix
BreezeVoiceSelector|BriaAddObject|BriaEraseByText|BriaEraseForeground|BriaEraser|BriaExpandImage
BriaGenFill|BriaGuidedRemoveBackground|BriaImageEditNode|BriaImageGenerate|BriaIncreaseResolution
BriaProductHolding|BriaRelight|BriaRemoveImageBackground|BriaRemoveVideoBackground
BriaReplaceImageBackground|BriaReplaceObject|BriaReseason|BriaRestorePhoto|BriaTransparentVideoBackground
BriaVideoEraser|BriaVideoGreenScreen|BriaVideoReplaceBackground|BriaVirtualTryOn|BuildJsonPromptIdeogram
BuildPoseFile|ByteDance2DraftToFinalVideoNode|ByteDance2FirstLastFrameNode|ByteDance2ReferenceNodeV2
ByteDance2TextToVideoNode|ByteDanceCreateImageAsset|ByteDanceCreateVideoAsset|ByteDanceFirstLastFrameNode
ByteDanceImageToVideoNode|ByteDanceSeedAudio|ByteDanceSeedNode|ByteDanceSeedreamLayerSeparationNodeV2
ByteDanceSeedreamNode|ByteDanceSeedreamNodeV3|ByteDanceTextToVideoNode|ByteDanceVideoEnhanceNode
CFGGuider|CFGNorm|CFGOverride|CFGZeroStar|CLIPAttentionMultiply|CLIPLoader|CLIPMergeAdd|CLIPMergeSimple
CLIPMergeSubtract|CLIPSave|CLIPSetLastLayer|CLIPTextEncode|CLIPTextEncodeControlnet|CLIPTextEncodeFlux
CLIPTextEncodeHiDream|CLIPTextEncodeHunyuanDiT|CLIPTextEncodeKandinsky5|CLIPTextEncodeLumina2
CLIPTextEncodePixArtAlpha|CLIPTextEncodeSD3|CLIPTextEncodeSDXL|CLIPTextEncodeSDXLRefiner|CLIPVisionEncode
CLIPVisionLoader|CameraAngle|Canny|CaseConverter|CheckpointLoader|CheckpointLoaderSimple|CheckpointSave
ChromaRadianceOptions|ClarityCrystalUpscaleNode|ClarityCrystalVideoUpscaleNode|ClaudeNode|ColorToRGBInt
ColorTransfer|ComboOptionTestNode|ComfyAndNode|ComfyCloudFlux2TextToImageNode
ComfyCloudMiniMaxH3FirstLastFrameToVideoNode|ComfyCloudMiniMaxH3ImageToVideoNode
ComfyCloudMiniMaxH3ReferenceToVideoNode|ComfyCloudMiniMaxH3TextToVideoNode
ComfyCloudMiniMaxMusic3TextToAudioNode|ComfyCloudZImageTurboNode|ComfyMathExpression|ComfyNotNode
ComfyNumberConvert|ComfyOrNode|ComfySoftSwitchNode|ComfySwitchNode|ConcatenateVideo|ConditioningAverage
ConditioningCombine|ConditioningConcat|ConditioningLoader|ConditioningMultiply|ConditioningSetArea
ConditioningSetAreaPercentage|ConditioningSetAreaPercentageVideo|ConditioningSetAreaStrength
ConditioningSetMask|ConditioningSetTimestepRange|ConditioningStableAudio|ConditioningZeroOut|Consumer
ContextWindowsManual|ControlNetApply|ControlNetApplyAdvanced|ControlNetApplySD3
ControlNetInpaintingAliMamaApply|ControlNetLoader|ConvertArrayToString|ConvertDictionaryToString
ConvertStringToComboNode|CosmosImageToVideoLatent|CosmosPredict2ImageToVideoLatent|CreateBoundingBoxes
CreateCameraInfo|CreateCameraOrbit|CreateList|CreateVideo|CropByBBoxes|CropMask|CurrentNode|CurveEditor
CustomCombo|DA3GeometryToMesh|DA3GeometryToPointCloud|DA3Inference|DA3Render|DCTestNode|DecimateMesh
DiffControlNetLoader|DifferentialDiffusion|DiffusersLoader|DisableNoise|DisabledTarget|DisabledV1
DrawBBoxes|DualCFGGuider|DualCLIPLoader|DualModelGuider|EasyCache|ElevenLabsAudioIsolation
ElevenLabsInstantVoiceClone|ElevenLabsSpeechToSpeech|ElevenLabsSpeechToText|ElevenLabsTextToDialogue
ElevenLabsTextToSoundEffects|ElevenLabsTextToSpeech|ElevenLabsVoiceSelector|EmptyARVideoLatent
EmptyAceStep1.5LatentAudio|EmptyAceStepLatentAudio|EmptyAudio|EmptyChromaRadianceLatentImage
EmptyCosmosLatentVideo|EmptyFlux2LatentImage|EmptyHiDreamO1LatentImage|EmptyHunyuanImageLatent
EmptyHunyuanLatentVideo|EmptyImage|EmptyLTXVLatentVideo|EmptyLatentAudio|EmptyLatentHunyuan3Dv2
EmptyLatentImage|EmptyMiniMaxH3LatentAV|EmptyMiniMaxMusic3LatentAudio|EmptyMochiLatentVideo
EmptyQwenImageLayeredLatentImage|EmptySD3LatentImage|EmptyTrellis2LatentStructure|EmptyYuE2LatentAudio
EndLoop|Epsilon Scaling|ExponentialScheduler|ExtendIntermediateSigmas|FeatherMask|File3DToSplat|FillHoles
FishAudioInstantVoiceClone|FishAudioSpeechToText|FishAudioTextToSpeech|FishAudioVoiceSelector|FlipSigmas
Flux2ImageNode|Flux2ProImageNode|Flux2Scheduler|Flux3ImageNode|Flux3LayoutPrompt|Flux3MaxVideoNodeBase
Flux3VideoNodeBase|FluxDisableGuidance|FluxEraseNode|FluxGuidance|FluxKVCache|FluxKontextImageScale
FluxKontextMultiReferenceLatentMethod|FluxKontextProImageNode|FluxProExpandNode|FluxProFillNode
FluxProUltraImageNode|FluxVTONode|FluxVideoEditNode|FluxVideoUpscaleNode|FluxVideoUpscaleNodeV2
FrameInterpolate|FrameInterpolationModelLoader|FreSca|FreeU|FreeU_V2|GITSScheduler|GLIGENLoader
GLIGENTextBoxApply|GLSLShader|GeminiImage2Node|GeminiImageNode|GeminiInputFiles|GeminiNanoBanana2
GeminiNanoBanana2V2|GeminiNode|GeminiNodeV2|GeminiNodeV3|GeminiVideoOmni|GeminiVideoOmniV2|GenerateTracks
GetAudioLatentSize|GetICLoRAParameters|GetImageSize|GetItemFromList|GetLatentSize|GetMeshInfo
GetSplatCount|GetVideoComponents|GrokImageEditNode|GrokImageEditNodeV2|GrokImageNode|GrokVideoEditNode
GrokVideoExtendNode|GrokVideoNode|GrokVideoReferenceNode|Group|GrowMask|HappyHorseImageToVideoApi
HappyHorseReferenceVideoApi|HappyHorseTextToVideoApi|HappyHorseVideoEditApi|HeyGenAvatarVideoNode
HeyGenCreateAvatarNode|HeyGenImageToVideoNode|HeyGenReferenceToVideoNode|HeyGenTalkingPhotoNode
HeyGenTextToSpeechNode|HeyGenVideoTranslateNode|HeyGenVoiceCloneNode|HeyGenVoiceTextToSpeechNode
HiDreamO1PatchSeamSmoothing|HiDreamO1ReferenceImages|HiggsfieldMotionTransferNode
HitPawGeneralImageEnhance|HitPawVideoEnhance|Hunyuan3Dv2Conditioning|Hunyuan3Dv2ConditioningMultiView
HunyuanImageEditApi|HunyuanImageTextToImageApi|HunyuanImageToVideo|HunyuanRefinerLatent
HunyuanVideo15ImageToVideo|HunyuanVideo15LatentUpscaleWithModel|HunyuanVideo15SuperResolution
HunyuanVideoImageToVideo|HyperTile|HypernetworkLoader|Ideogram4Scheduler|IdeogramEditApi|IdeogramPImage
IdeogramPreciseEditApi|IdeogramTextToImageApi|IdeogramV3|IdeogramV4|IdeogramVectorGenerateApi
IdeogramVectorImageToSVGApi|ImageAddNoise|ImageBatch|ImageBlend|ImageBlur|ImageColorSpace
ImageColorToMask|ImageCompare|ImageCompositeMasked|ImageCompositor|ImageCrop|ImageCropToMask|ImageCropV2
ImageFlip|ImageFromBatch|ImageHistogram|ImageInvert|ImageMergeTileList|ImageOnlyCheckpointLoader
ImageOnlyCheckpointSave|ImagePadForOutpaint|ImageProcessingNode|ImageQuantize|ImageRGBToYUV|ImageRotate
ImageSDRToHDR|ImageScale|ImageScaleBy|ImageScaleToMaxDimension|ImageScaleToTotalPixels|ImageSharpen
ImageStitch|ImageToMask|ImageUpscaleWithModel|ImageYUVToRGB|InpaintModelConditioning
InstructPixToPixConditioning|InvertBooleanNode|InvertMask|JoinAudioChannels|JoinImageWithAlpha
JsonExtractString|KSampler|KSamplerAdvanced|KSamplerSelect|Kandinsky5ImageToVideo|KarrasScheduler
Kling4EditVideoNode|Kling4ImageToVideoNode|Kling4ReferenceToVideoNode|Kling4TextToVideoNode
KlingAvatarNode|KlingFirstLastFrameNode|KlingImage2VideoNode|KlingImageGenerationNode
KlingImageToVideoWithAudio|KlingLipSyncAudioToVideoNode|KlingLipSyncTextToVideoNode|KlingMotionControl
KlingOmniProEditVideoNode|KlingOmniProFirstLastFrameNode|KlingOmniProImageNode
KlingOmniProImageToVideoNode|KlingOmniProTextToVideoNode|KlingOmniProVideoToVideoNode
KlingStartEndFrameNode|KlingTextToVideoNode|KlingTextToVideoWithAudio|KlingTryOnNode|KlingVideoNode
Krea2ImageNode|Krea2StyleReferenceNode|Krea3ImageNode|LTXAVTextEncoderLoader|LTXVAddGeneratedKeyframes
LTXVAddGuide|LTXVAddLatentGuide|LTXVAudioVAEDecode|LTXVAudioVAELoader|LTXVConcatAVLatent|LTXVConditioning
LTXVCropGuides|LTXVDualCFGGuider|LTXVDurationPredictor|LTXVEmptyLatentAudio|LTXVFreezeLatent
LTXVGeneratedKeyframesToGuides|LTXVImgToVideo|LTXVImgToVideoInplace|LTXVLatentUpsampler
LTXVModalityGuidance|LTXVPreprocess|LTXVReferenceAudio|LTXVScheduler|LTXVSeparateAVLatent
LTXVSeparateGeneratedKeyframes|LTXVSpatioTemporalGuidance|LaplaceScheduler|LatentAdd|LatentApplyOperation
LatentApplyOperationCFG|LatentBatch|LatentBatchSeedBehavior|LatentBlend|LatentComposite
LatentCompositeMasked|LatentConcat|LatentCrop|LatentCut|LatentCutToBatch|LatentFlip|LatentFromBatch
LatentInterpolate|LatentMultiply|LatentOperationBlend|LatentOperationSharpen
LatentOperationTonemapReinhard|LatentRotate|LatentSubtract|LatentUpscale|LatentUpscaleBy
LatentUpscaleModelLoader|LayersFromBoundingBoxes|LazyCache|LeakTest\|LegacyNode|Load3D|Load3DAdvanced
LoadAudio|LoadBackgroundRemovalModel|LoadDA3Model|LoadImage|LoadImageDataSetFromFolder|LoadImageMask
LoadImageOutput|LoadImageTextDataSetFromFolder|LoadLatent|LoadLoraModel|LoadLoraTextEncoder
LoadMediaPipeFaceLandmarker|LoadMoGeModel|LoadTrainingDataset|LoadVideo|LoadVideoDataSetFromFolder
LoadVideoTextDataSetFromFolder|LoopIteration|LoopProgress|LoopResult|LoraLoader|LoraLoaderBypass
LoraLoaderBypassModelOnly|LoraLoaderModelOnly|LoraModelLoader|LoraSave|LossGraphNode|LotusConditioning
LtxApi25AudioToVideo|LtxApi25ImageToVideo|LtxApi25TextToVideo|LtxApiVideoAlphaGen|LtxApiVideoReframe
LtxApiVideoSdrToHdr|LtxApiVideoUpscale|LumaImageEditNode2|LumaImageNode2|LumaRay32ExtendVideoNode
LumaRay32ImageToVideoNode|LumaRay32KeyframeNode|LumaRay32KeyframesToVideoNode|LumaRay32TextToVideoNode
LumaRay32VideoEditNode|LumaRay32VideoReframeNode|MOSSImageControl|MOSSImageLayoutPrompt
MagnificImageRelightNode|MagnificImageSkinEnhancerNode|MagnificImageStyleTransferNode
MagnificImageUpscalerCreativeNode|MagnificImageUpscalerPreciseV2Node|Mahiro|MakeTrainingDataset
ManualSigmas|MarigoldV2PostProcess|MarkdownNote|MaskComposite|MaskPreview|MaskToImage
MediaPipeFaceLandmarker|MediaPipeFaceMask|MediaPipeFaceMeshVisualize|MergeMeshes|MergeSplat
MeshSmoothNormals|MeshTextureToImage|MeshToFile3D|MeshyAnimateModelNode|MeshyImageToModelNode
MeshyImageToModelSmartTopologyNode|MeshyMultiImageToModelNode|MeshyRefineNode|MeshyRigModelNode
MeshyTextToModelNode|MeshyTextToModelSmartTopologyNode|MeshyTextureMultiViewNode|MeshyTextureNode
MetaMuseImageEditApi|MetaMuseImageTextToImageApi|MiniMaxH3AddGuide|MiniMaxH3FunControlNetApply
MiniMaxH3ImageConditioning|MiniMaxH3ImageToVideo|MiniMaxH3LatentUpscaleWithModel
MiniMaxH3ReferenceToVideo|MiniMaxH3SigmaShift|MiniMaxMusic3TextEncode|MinimaxH3ImageNode
MinimaxHailuo03ContextIRNode|MinimaxHailuo03FirstLastFrameNode|MinimaxHailuo03MaxCameraControlNode
MinimaxHailuo03MaxExtendVideoNode|MinimaxHailuo03MaxInsertVideoNode|MinimaxHailuo03MaxLipSyncNode
MinimaxHailuo03MaxRecastNode|MinimaxHailuo03MaxReframeVideoNode|MinimaxHailuo03ReferenceNode
MinimaxHailuo03RegenerateNode|MinimaxHailuo03TextToVideoNode|MinimaxHailuoVideoNode
MinimaxImageToVideoNode|MinimaxSubjectToVideoNode|MinimaxTextToVideoNode|MireloTextToSFX|MireloVideoToSFX
MoGeGeometryToFOV|MoGeInference|MoGePanoramaInference|MoGePointMapToMesh|MoGeRender|ModelAttentionBackend
ModelComputeDtype|ModelMergeAdd|ModelMergeAuraflow|ModelMergeBlocks|ModelMergeCosmos14B
ModelMergeCosmos7B|ModelMergeCosmosPredict2_14B|ModelMergeCosmosPredict2_2B|ModelMergeFlux1
ModelMergeKrea2|ModelMergeLTXV|ModelMergeMOSSImage|ModelMergeMochiPreview|ModelMergeQwenImage
ModelMergeSD1|ModelMergeSD2|ModelMergeSD35_Large|ModelMergeSD3_2B|ModelMergeSDXL|ModelMergeSimple
ModelMergeSubtract|ModelMergeWAN2_1|ModelNoiseScale|ModelPatchLoader|ModelSamplingAuraFlow
ModelSamplingContinuousEDM|ModelSamplingContinuousV|ModelSamplingDiscrete|ModelSamplingFlux
ModelSamplingLTXV|ModelSamplingSD3|ModelSamplingStableCascade|ModelSave|Morphology|MultiGPU_Options
MultiGPU_WorkUnits|NAGuidance|NormalizeVideoLatentStart|Note|OpenAIChatConfig|OpenAIChatNode
OpenAIGPTImage1|OpenAIGPTImageGIFNode|OpenAIGPTImageNodeV2|OpenAIGPTImageNodeV3|OpenAIGPTImageSVGNode
OpenAIInputFiles|OpenRouterImageNode|OpenRouterLLMNode|OpticalFlowLoader|OptimalStepsScheduler
PH3ImageConditioning|PadImageNode|PaintMesh|Painter|PatchModelAddDownscale|PatinaMaterialExtractNode
PatinaMaterialNode|PatinaPBRMapsNode|PerpNeg|PerpNegGuider|PerturbedAttentionGuidance|PhotoMakerEncode
PhotoMakerLoader|PiDConditioning|PikaH3FlexReferenceVideoNode|PikaH3FlexVideoNode|PikaMusicNode
PikaSFXNode|PikaSoundtrackNode|Pixal3DConditioning|Pixal3DMultiViewConditioning|PixverseImageToVideoNode
PixverseTemplateNode|PixverseTextToVideoNode|PixverseTransitionVideoNode|PixverseV6ExtendVideoNode
PixverseV6FirstLastFrameNode|PixverseV6FusionVideoNode|PixverseV6ImageToVideoNode
PixverseV6TextToVideoNode|PolyexponentialScheduler|PorterDuffImageComposite|Preview3D|Preview3DAdvanced
PreviewAny|PreviewAudio|PreviewGaussianSplat|PreviewImage|PreviewPointCloud|PrimitiveBoolean
PrimitiveBoundingBox|PrimitiveFloat|PrimitiveInt|PrimitiveNode|PrimitiveString|PrimitiveStringMultiline
PrunaImageToVideoNode|PrunaPVideoUpscaleNode|PrunaTextToVideoNode|QuadrupleCLIPLoader
QuiverImageToSVGNode|QuiverImageToSVGNodeV2|QuiverTextToSVGNode|QuiverTextToSVGNodeV2|QwenImage21Cache
QwenImageDiffsynthControlnet|QwenImageEditApi|QwenImageTextToImageApi|RTDETR_detect|RandomNoise
RebatchImages|RebatchLatents|RecordAudio|RecraftColorRGB|RecraftControls|RecraftCreateStyleNode
RecraftCrispUpscaleNode|RecraftImageInpaintingNode|RecraftImageToImageNode|RecraftRemoveBackgroundNode
RecraftReplaceBackgroundNode|RecraftStyleV3InfiniteStyleLibrary|RecraftStyleV3RealisticImage
RecraftTextToImageNode|RecraftTextToVectorNode|RecraftV4CreateStyleNode|RecraftV4TextToImageNode
RecraftV4TextToVectorNode|RecraftVectorizeImageNode|ReferenceLatent|ReferenceMultiLatent
ReferenceTimbreAudio|RegexExtract|RegexMatch|RegexReplace|RelayVSRConditioning|RemeshMesh
RemoveBackground|RenderMesh|RenderSplat|RenderUVAtlas|RenormCFG|RepeatImageBatch|RepeatLatentBatch
ReplaceVideoLatentFrames|Reroute|RescaleCFG|ResizeAndPadImage|ResizeImageMaskNode|ResolutionBucket
ResolutionSelector|ReveImageCreateNode|ReveImageEditNode|ReveImageRemixNode|Rodin3D_Detail|Rodin3D_Gen2
Rodin3D_Gen25_Image|Rodin3D_Gen25_Text|Rodin3D_Regular|Rodin3D_Sketch|Rodin3D_Smooth|RotateMesh
RunwayAleph2KeyframeNode|RunwayAleph2PromptImageNode|RunwayAleph2VideoToVideoNode
RunwayFirstLastFrameNode|RunwayImageToVideoNodeGen3a|RunwayImageToVideoNodeGen4|RunwayTextToImageNode
SAM3DBody_FaceExpression|SAM3DBody_Loader|SAM3DBody_Predict|SAM3DBody_Render|SAM3DBody_Smooth|SAM3_Detect
SAM3_TrackPreview|SAM3_TrackToMask|SAM3_VideoTrack|SCAIL2ColoredMask|SDPoseDrawKeypoints|SDPoseFaceBBoxes
SDPoseKeypointExtractor|SDTurboScheduler|SD_4XUpscale_Conditioning|SUPIRApply|SV3D_Conditioning
SVD_img2vid_Conditioning|SamplerARVideo|SamplerCustom|SamplerCustomAdvanced|SamplerDPMAdaptative
SamplerDPMPP_2M_SDE|SamplerDPMPP_2S_Ancestral|SamplerDPMPP_3M_SDE|SamplerDPMPP_SDE|SamplerER_SDE
SamplerEulerAncestral|SamplerEulerAncestralCFGPP|SamplerEulerCFGpp|SamplerLCM|SamplerLCMUpscale
SamplerLMS|SamplerSASolver|SamplerSEEDS2|SamplingPercentToSigma|Save3DAdvanced|SaveAnimatedPNG
SaveAnimatedWEBP|SaveAudio|SaveAudioAdvanced|SaveAudioMP3|SaveAudioOpus|SaveConditioning|SaveGLB
SaveGaussianSplat|SaveImage|SaveImageAdvanced|SaveImageDataSetToFolder|SaveImageTextDataSetToFolder
SaveImageWebsocket|SaveLatent|SaveLoRA|SavePointCloud|SaveSVGNode|SaveText|SaveTrainingDataset|SaveVideo
SaveWEBM|ScaleROPE|SeedNode|SeedVR2Conditioning|SeedVR2PostProcessing|SeedVR2Preprocess
SeedVR2TemporalChunk|SeedVR2TemporalMerge|SelectCLIPDevice|SelectModelDevice|SelectVAEDevice
SelfAttentionGuidance|SenseNovaConditioning|SenseNovaInterleaveDecode|SenseNovaSamplingOptions
SetFirstSigma|SetLatentNoiseMask|SetUnionControlNetType|SheetSage2AudioToABC|ShuffleImageTextDataset
ShuffleVideoDataset|ShuffleVideoTextDataset|Sink|SkipLayerGuidanceDiT|SkipLayerGuidanceDiTSimple
SkipLayerGuidanceSD3|SolidMask|SoniloMusic15|SoniloTextToMusic|SoniloVideoToMusic|Source|SplatToFile3D
SplatToMesh|SplitAudioChannels|SplitImageChannels|SplitImageToTileList|SplitImageWithAlpha|SplitSigmas
SplitSigmasDenoise|StableCascade_EmptyLatentImage|StableCascade_StageB_Conditioning
StableCascade_StageC_VAEEncode|StableCascade_SuperResolutionControlnet|StableZero123_Conditioning
StableZero123_Conditioning_Batched|StartLoop|StringCompare|StringConcatenate|StringContains|StringFormat
StringLength|StringReplace|StringSubstring|StringTrim|StubConstantImage|StubFloat|StubImage|StubInt
StubMask|StyleModelApply|StyleModelLoader|SyncLipSyncNode|SyncTalkingImageNode|T5TokenizerOptions|TCFG
TemporalScoreRescaling|Tencent3DPartNode|Tencent3DTextureEditNode|TencentHyNode|TencentImageToModelNode
TencentModelTo3DUVNode|TencentSmartTopologyNode|TencentTextToModelNode|TestAccumulateNode
TestAccumulationGetItemNode|TestAccumulationGetLengthNode|TestAccumulationHeadNode
TestAccumulationSetItemNode|TestAccumulationTailNode|TestAccumulationToListNode|TestAsyncBatchProcessing
TestAsyncConcurrentLimit|TestAsyncError|TestAsyncLazyCheck|TestAsyncProgressUpdate|TestAsyncResourceUser
TestAsyncTimeout|TestAsyncValidation|TestAsyncValidationError|TestBoolOperationNode
TestCapturePassthroughSecond|TestCustomIsChanged|TestCustomValidation1|TestCustomValidation2
TestCustomValidation3|TestCustomValidation4|TestCustomValidation5|TestDisabledNode
TestDynamicAsyncGeneration|TestDynamicDependencyCycle|TestExecutedNodeIdsBlocking
TestExecutedNodeIdsChild|TestExecutedNodeIdsExpander|TestExecutionBlocker|TestExpandsToDisabledNode
TestFloatConditions|TestForLoopClose|TestForLoopOpen|TestIntConditions|TestIntMathOperation
TestIsChangedWithConstants|TestLazyMixImages|TestListToAccumulationNode|TestMakeListNode
TestMixedExpansionReturns|TestOutputNodeWithSocketOutput|TestParallelSleep|TestSamplingInExpansion
TestSleep|TestStringConditions|TestSyncError|TestSyncProgressUpdate|TestToBoolNode|TestVariadicAverage
TestWhileLoopClose|TestWhileLoopOpen|TextEncodeAceStepAudio|TextEncodeAceStepAudio1.5|TextEncodeBooguEdit
TextEncodeFibo2|TextEncodeHunyuanVideo_ImageToVideo|TextEncodeJoyImageEdit|TextEncodeMOSSImageEdit
TextEncodeMageFlowEdit|TextEncodeMingImageEdit|TextEncodeQwenImage21|TextEncodeQwenImageEdit
TextEncodeQwenImageEditPlus|TextEncodeZImageOmni|TextGenerateV2|TextOverlay|TextProcessingNode
ThresholdMask|TomePatchModel|TopazImageEnhance|TopazImageEnhanceV2|TopazVideoEnhance|TopazVideoEnhanceV2
TorchCompileModel|TrainLoraNode|TransformSplat|Trellis2Conditioning|Trellis2ShapeStage
Trellis2TextureStage|Trellis2UpsampleStage|TrimAudioDuration|TrimVideoLatent|TripleCLIPLoader
TripoConversionNode|TripoEditMultiviewNode|TripoImageToModelNode|TripoImageToModelNodeV2
TripoImageToMultiviewNode|TripoImportModelNode|TripoMeshCompleteNode|TripoMultiviewToModelNode
TripoP1ImageToModelNode|TripoP1MultiviewToModelNode|TripoP1TextToModelNode|TripoPSeriesImageToModelNode
TripoPSeriesMultiviewToModelNode|TripoPSeriesTextToModelNode|TripoRetargetNode|TripoRetopologyNode
TripoRigCheckNode|TripoRigNode|TripoSegmentNode|TripoSmartSegmentNode|TripoSplatConditioning
TripoSplatPreprocessImage|TripoSplatSamplingPreview|TripoTextToModelNode|TripoTextToModelNodeV2
TripoTextureNode|TripoTextureNodeV2|UNETLoader|UNetCrossAttentionMultiply|UNetSelfAttentionMultiply
UNetTemporalAttentionMultiply|USOStyleReference|UltraTexConditioning|UnwrapMesh|UpscaleModelLoader
VAEComputeDtype|VAEDecode|VAEDecodeAudio|VAEDecodeAudioTiled|VAEDecodeHunyuan3D|VAEDecodeTiled
VAEDecodeTripoSplat|VAEEncode|VAEEncodeAudio|VAEEncodeForInpaint|VAEEncodeTiled|VAELoader|VAESave
VOIDInpaintConditioning|VOIDQuadmaskPreprocess|VOIDSampler|VOIDWarpedNoise|VOIDWarpedNoiseSource
VPScheduler|VSRDecode|VaeDecodeShapeTrellis|VaeDecodeStructureTrellis2|VaeDecodeTextureTrellis|Validator
VeedCleanAudioNode|VeedCleanVideoAudioNode|VeedDubbingNode|VeedFabricNode|VeedLipsyncNode
VeedRemoveVideoBackgroundNode|Veo3FirstLastFrameNode|Veo3VideoGenerationNode|Video Slice|VideoCrop
VideoDeflicker|VideoFrameSample|VideoLinearCFGGuidance|VideoRandomTemporalCrop|VideoTemporalCrop
VideoTriangleCFGGuidance|VideoTrim|Vidu2ImageToVideoNode|Vidu2ReferenceVideoNode|Vidu2StartEndToVideoNode
Vidu2TextToVideoNode|Vidu3ImageToVideoNode|Vidu3StartEndToVideoNode|Vidu3TextToVideoNode
Vidu4ImageToVideoNode|Vidu4ReferenceVideoNode|ViduExtendVideoNode|ViduImageToVideoNode
ViduMultiFrameVideoNode|ViduReferenceVideoNode|ViduStartEndToVideoNode|ViduTextToVideoNode|VoxelToMesh
VoxelToMeshBasic|Wan22FunControlToVideo|Wan22ImageToVideoLatent|Wan2ImageToVideoApi|Wan2ReferenceVideoApi
Wan2TextToVideoApi|Wan2VideoContinuationApi|Wan2VideoEditApi|Wan3ImageToVideoApi|Wan3ReferenceToVideoApi
WanAnimate2Cache|WanAnimate2ToVideo|WanAnimateToVideo|WanCameraEmbedding|WanCameraImageToVideo
WanDancerEncodeAudio|WanDancerPadKeyframes|WanDancerPadKeyframesList|WanDancerVideo
WanFirstLastFrameToVideo|WanFunControlToVideo|WanFunInpaintToVideo|WanHuMoImageToVideo|WanImageToImageApi
WanImageToVideo|WanImageToVideoApi|WanInfiniteTalkToVideo|WanMoveConcatTrack|WanMoveTrackToVideo
WanMoveTracksFromCoords|WanMoveVisualizeTracks|WanPhantomSubjectToVideo|WanReferenceVideoApi
WanSCAILToVideo|WanSoundImageToVideo|WanSoundImageToVideoExtend|WanTextToImageApi|WanTextToVideoApi
WanTrackToVideo|WanUni3CControlnetApply|WanVaceToVideo|WavespeedFlashVSRNode|WavespeedImageUpscaleNode
WeVisOneDetect|WebcamCapture|WeldVertices|YuE2GenerateABC|YuE2GenerateMusic|ZImageFunControlnet
_ComfyCloudMageFlowNode|unCLIPCheckpointLoader|unCLIPConditioning|wanBlockSwap"#.split(['|', '\n']).collect()
});

fn builtin(class: &str) -> bool {
    CORE_NODES.contains(class)
}

fn filenames<'a>(value: &'a Value, names: &mut Vec<&'a str>) {
    match value {
        Value::String(s) => {
            let name = s.trim();
            if let Some((_, ext)) = name.rsplit_once('.') {
                if matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "safetensors" | "ckpt" | "pt" | "pth" | "gguf" | "onnx" | "bin" | "sft"
                ) {
                    names.push(name);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|v| filenames(v, names)),
        Value::Object(o) => o.values().for_each(|v| filenames(v, names)),
        _ => {}
    }
}

fn path_key(s: &str) -> String {
    s.replace('\\', "/").trim_start_matches("./").to_lowercase()
}

pub fn scan_workflow_models(
    workflow_name: &str,
    workflow_json: &str,
    drive_models: &[DriveModelItem],
) -> Result<WorkflowScanResult, String> {
    let document: Value =
        serde_json::from_str(workflow_json).map_err(|e| format!("Invalid workflow JSON: {e}"))?;
    let root = document
        .get("prompt")
        .filter(|v| v.is_object() && document.get("nodes").is_none())
        .unwrap_or(&document);
    let mut nodes = vec![];
    let mut subgraph_ids = HashSet::new();
    if let Some(ui) = root.get("nodes") {
        let mut lists = vec![ui.as_array().ok_or("Workflow nodes must be an array")?];
        if let Some(subgraphs) = root
            .pointer("/definitions/subgraphs")
            .and_then(Value::as_array)
        {
            for sub in subgraphs {
                if let Some(id) = sub["id"].as_str() {
                    subgraph_ids.insert(id);
                }
                if let Some(list) = sub["nodes"].as_array() {
                    lists.push(list);
                }
            }
        }
        for node in lists.into_iter().flatten() {
            nodes.push((
                node["type"].as_str().ok_or("Missing UI node type")?,
                node.pointer("/properties/cnr_id").and_then(Value::as_str),
                &node["widgets_values"],
                &node["inputs"],
            ));
        }
    } else {
        for node in root
            .as_object()
            .ok_or("Expected a workflow object")?
            .values()
        {
            if let Some(class) = node["class_type"].as_str() {
                nodes.push((class, None, &node["inputs"], &Value::Null));
            }
        }
        if nodes.is_empty() {
            return Err("Expected ComfyUI nodes or API class_type entries".into());
        }
    }
    let node_count = nodes.len();
    let mut custom = BTreeSet::new();
    let mut seen = HashSet::new();
    let mut models = vec![];
    for (class, registry, values, extra) in nodes {
        if !subgraph_ids.contains(class)
            && !registry.map_or_else(
                || builtin(class),
                |id| matches!(id, "comfy-core" | "comfyui"),
            )
        {
            custom.insert(class.to_string());
        }
        let inferred = category(class);
        let mut names = vec![];
        filenames(values, &mut names);
        filenames(extra, &mut names);
        for name in names {
            let key = path_key(name);
            if !seen.insert((inferred, key.clone())) {
                continue;
            }
            let base = key.rsplit('/').next().unwrap_or(&key);
            let full = format!("{}/{key}", inferred.drive_subdir());
            let matched = drive_models
                .iter()
                .filter_map(|m| {
                    let path = path_key(&m.relativePath);
                    let score = if path == key || path == full {
                        0
                    } else if path.rsplit('/').next() == Some(base) || path_key(&m.filename) == base
                    {
                        if m.category == inferred {
                            1
                        } else {
                            2
                        }
                    } else {
                        return None;
                    };
                    Some((score, m))
                })
                .min_by_key(|(score, m)| (*score, &m.relativePath))
                .map(|(_, m)| m);
            let suggested = matched.is_none().then(|| {
                let mut url = Url::parse("https://huggingface.co/models").unwrap();
                url.query_pairs_mut()
                    .append_pair("search", name.rsplit(['/', '\\']).next().unwrap_or(name));
                url.to_string()
            });
            models.push(WorkflowModelRequirement {
                filename: name.into(),
                category: inferred,
                nodeType: class.into(),
                presentOnDrive: matched.is_some(),
                matchedDrivePath: matched.map(|m| m.relativePath.clone()),
                suggestedUrl: suggested,
            });
        }
    }
    let present_count = models.iter().filter(|m| m.presentOnDrive).count();
    Ok(WorkflowScanResult {
        workflowName: workflow_name.into(),
        nodeCount: node_count,
        requiredCustomNodes: custom.into_iter().collect(),
        presentCount: present_count,
        missingCount: models.len() - present_count,
        models,
    })
}

static QUANT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[^a-z0-9])((?:UD[-_])?(?:I?Q[1-8](?:_[a-z0-9]+)*|BF16|F16|FP16|F32|FP32))",
    )
    .unwrap()
});
static SHARD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)-(\d+)-of-(\d+)$").unwrap());
const CONTEXTS: [u32; 3] = [32768, 16384, 8192];

fn precision(quant: &str) -> (bool, u8, u8) {
    if let Some((_, tail)) = quant.split_once('Q') {
        let bits = tail.as_bytes()[0] - b'0';
        let grade = match quant.rsplit('_').next() {
            Some("XXS") => 0,
            Some("XS") => 1,
            Some("S") => 2,
            Some("K") => 4,
            Some("M" | "1") => 5,
            Some("L") => 6,
            Some("XL") => 7,
            _ => 3,
        };
        (true, bits, grade)
    } else {
        (false, if quant.ends_with("32") { 32 } else { 16 }, 0)
    }
}

fn vram_mib(bytes: u64, ctx: u32) -> u32 {
    u32::try_from(bytes.div_ceil(1 << 20))
        .unwrap_or(u32::MAX)
        .saturating_add(ctx / 4 + 768)
}

fn inspect_metadata(
    repo_id: &str,
    target: u32,
    meta: &Value,
) -> Result<GgufRepoInspection, String> {
    struct Shard<'a> {
        index: u32,
        count: u32,
        path: &'a str,
        size: u64,
    }
    let mut groups: BTreeMap<(String, &str), Vec<Shard>> = BTreeMap::new();
    let mut projectors = vec![];
    for file in meta["siblings"]
        .as_array()
        .ok_or("Hub response lacks a file listing")?
    {
        let Some(path) = file["rfilename"].as_str() else {
            continue;
        };
        let Some((stem, ext)) = path.rsplit_once('.') else {
            continue;
        };
        if !ext.eq_ignore_ascii_case("gguf") {
            continue;
        }
        let size = file["size"]
            .as_u64()
            .filter(|s| *s > 0)
            .or_else(|| file.pointer("/lfs/size").and_then(Value::as_u64))
            .unwrap_or(0);
        if stem.to_ascii_lowercase().contains("mmproj") {
            projectors.push((path, size));
            continue;
        }
        let (family, index, count) = if let Some(parts) = SHARD.captures(stem) {
            (
                &stem[..parts.get(0).unwrap().start()],
                parts[1].parse::<u32>().unwrap_or(0),
                parts[2].parse::<u32>().unwrap_or(0),
            )
        } else {
            (stem, 1, 1)
        };
        let Some(label) = QUANT
            .captures_iter(family)
            .filter(|m| {
                family
                    .as_bytes()
                    .get(m.get(1).unwrap().end())
                    .is_none_or(|b| !b.is_ascii_alphanumeric())
            })
            .last()
        else {
            continue;
        };
        let quant = label[1].to_ascii_uppercase().replace("UD_", "UD-");
        groups.entry((quant, family)).or_default().push(Shard {
            index,
            count,
            path,
            size,
        });
    }
    projectors.sort();
    let mut variants = vec![];
    for ((quant, family), mut shards) in groups {
        shards.sort_by_key(|s| s.index);
        if shards.iter().enumerate().any(|(i, s)| {
            s.index as usize != i + 1 || s.count as usize != shards.len() || s.size == 0
        }) {
            continue;
        }
        let weights = shards
            .iter()
            .try_fold(0u64, |a, s| a.checked_add(s.size))
            .ok_or("GGUF weights too large")?;
        let parent = family.rsplit_once('/').map(|s| s.0);
        let projector = projectors
            .iter()
            .min_by_key(|(p, size)| (p.rsplit_once('/').map(|s| s.0) != parent, *size, *p));
        if projector.is_some_and(|(_, size)| *size == 0) {
            continue;
        }
        let bytes = weights.saturating_add(projector.map_or(0, |p| p.1));
        let trained = meta
            .pointer("/gguf/context_length")
            .and_then(Value::as_u64)
            .unwrap_or(32768)
            .max(8192);
        let ctx = CONTEXTS
            .into_iter()
            .filter(|ctx| u64::from(*ctx) <= trained)
            .find(|ctx| u64::from(vram_mib(bytes, *ctx)) * 100 <= u64::from(target) * 95);
        let recommended = ctx.unwrap_or(8192);
        variants.push(GgufVariantEstimate {
            quant,
            files: shards.iter().map(|s| s.path.into()).collect(),
            sizeBytes: weights,
            estimatedVramMib: vram_mib(bytes, recommended),
            recommendedCtx: recommended,
            kvCacheType: "q8_0".into(),
            fitsCurrentTarget: ctx.is_some(),
            isAutoSelected: false,
        });
    }
    variants.sort_by_key(|v| std::cmp::Reverse(precision(&v.quant)));
    let picked = variants.iter_mut().find(|v| v.fitsCurrentTarget).map(|v| {
        v.isAutoSelected = true;
        v.quant.clone()
    });
    Ok(GgufRepoInspection {
        repoId: repo_id.into(),
        targetVramMib: target,
        variants,
        mmprojFiles: projectors.iter().map(|p| p.0.into()).collect(),
        autoPickedQuant: picked,
    })
}

pub async fn inspect_hf_gguf_repo(
    client: &Client,
    repo_id: &str,
    target_vram_mib: u32,
    hf_token: Option<&str>,
) -> Result<GgufRepoInspection, String> {
    let parts: Vec<_> = repo_id.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|s| {
            s.is_empty()
                || s.starts_with('.')
                || !s
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        })
    {
        return Err("Expected a Hugging Face repo ID of the form owner/repo".into());
    }
    let mut request = client
        .get(format!("https://huggingface.co/api/models/{repo_id}"))
        .query(&[("blobs", "true")])
        .timeout(Duration::from_secs(30));
    if let Some(token) = hf_token.filter(|s| !s.is_empty()) {
        request = request.bearer_auth(token);
    }
    let metadata: Value = request
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    inspect_metadata(repo_id, target_vram_mib, &metadata)
}

fn settings_url(base: &str) -> Result<Url, String> {
    let mut url = Url::parse(base.trim()).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https") || !url.has_host() {
        return Err("Invalid DramaSmith URL".into());
    }
    url.set_path(&format!(
        "{}/api/settings/colab",
        url.path().trim_end_matches('/')
    ));
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

pub async fn check_dramasmith_reachable(client: &Client, dramasmith_base: &str) -> bool {
    let Ok(url) = settings_url(dramasmith_base) else {
        return false;
    };
    client
        .get(url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

const ENDPOINT_FIELDS: &[&str] = &[
    "colabEndpointUrl",
    "colab_endpoint_url",
    "baseUrl",
    "base_url",
    "endpointUrl",
    "endpoint_url",
    "colabUrl",
    "colab_url",
    "comfyuiUrl",
    "comfyui_url",
    "comfyUrl",
    "comfy_url",
    "serviceUrl",
    "service_url",
    "endpoint",
    "url",
];

fn saved_endpoint(config: &Value) -> Option<(&str, &Value)> {
    [
        config,
        &config["settings"],
        &config["colab"],
        &config["config"],
        &config["data"],
    ]
    .into_iter()
    .find_map(|object| {
        ENDPOINT_FIELDS
            .iter()
            .find_map(|key| object.get(*key).map(|v| (*key, v)))
    })
}

fn update_contract(spec: &Value) -> Option<(reqwest::Method, Value)> {
    let route = &spec["paths"]["/api/settings/colab"];
    for (verb, method) in [
        ("patch", reqwest::Method::PATCH),
        ("put", reqwest::Method::PUT),
        ("post", reqwest::Method::POST),
    ] {
        let schema = &route[verb]["requestBody"]["content"]["application/json"]["schema"];
        let schema = schema["$ref"]
            .as_str()
            .and_then(|r| spec.pointer(r.strip_prefix('#')?))
            .unwrap_or(schema);
        if schema["properties"].is_object() {
            return Some((method, schema["properties"].clone()));
        }
    }
    None
}

pub async fn sync_dramasmith_colab_url(
    client: &Client,
    dramasmith_base: &str,
    colab_endpoint_url: &str,
) -> Result<String, String> {
    let url = settings_url(dramasmith_base)?;
    let endpoint = Url::parse(colab_endpoint_url.trim()).map_err(|e| e.to_string())?;
    if !matches!(endpoint.scheme(), "http" | "https") || !endpoint.has_host() {
        return Err("Invalid Colab endpoint".into());
    }
    let endpoint = endpoint.as_str().trim_end_matches('/');
    let get = || client.get(url.clone()).timeout(Duration::from_secs(5));
    let prior: Value = get()
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let prior_field = saved_endpoint(&prior);
    if prior_field
        .and_then(|(_, v)| v.as_str())
        .is_some_and(|s| s.trim_end_matches('/') == endpoint)
    {
        return Ok(endpoint.into());
    }
    let spec_url = format!("{}/openapi.json", dramasmith_base.trim_end_matches('/'));
    let contract = match client
        .get(spec_url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r
            .json::<Value>()
            .await
            .ok()
            .and_then(|v| update_contract(&v)),
        _ => None,
    };
    let field = contract
        .as_ref()
        .and_then(|(_, props)| saved_endpoint(props).map(|(k, _)| k))
        .or_else(|| prior_field.map(|(k, _)| k))
        .ok_or("DramaSmith did not expose its Colab endpoint field")?;
    let mut body = json!({(field):endpoint});
    // Save only the Colab profile. A global service switch affects other projects.
    if let Some((_, props)) = &contract {
        for key in props.as_object().unwrap().keys() {
            let name = key.replace('_', "").to_ascii_lowercase();
            if [
                "applytoglobal",
                "setasglobal",
                "activate",
                "setactive",
                "setasactive",
                "applytoservice",
                "activateglobal",
                "applytoruntime",
                "applytovideoservice",
                "activatevideoservice",
                "applytosettings",
            ]
            .contains(&name.as_str())
            {
                body[key] = json!(false);
            }
        }
    } else {
        body["applyToGlobal"] = json!(false);
    }
    let request = |r: reqwest::RequestBuilder| r.timeout(Duration::from_secs(8)).json(&body).send();
    let method = contract
        .as_ref()
        .map(|(m, _)| m.clone())
        .unwrap_or(reqwest::Method::PUT);
    let mut response = request(client.request(method, url.clone()))
        .await
        .map_err(|e| e.to_string())?;
    if contract.is_none() && response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
        response = request(client.post(url.clone()))
            .await
            .map_err(|e| e.to_string())?;
    }
    response.error_for_status().map_err(|e| e.to_string())?;
    let after: Value = get()
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if saved_endpoint(&after)
        .and_then(|(_, v)| v.as_str())
        .is_none_or(|s| s.trim_end_matches('/') != endpoint)
    {
        return Err("DramaSmith did not confirm the saved Colab endpoint".into());
    }
    Ok(endpoint.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(path: &str, category: ModelCategory) -> DriveModelItem {
        DriveModelItem {
            filename: path.rsplit('/').next().unwrap().into(),
            relativePath: path.into(),
            category,
            sizeBytes: 100,
            updatedAt: None,
        }
    }

    #[test]
    fn ui_path_matches_precede_basename_and_custom_registry_is_respected() {
        let graph = json!({"nodes":[
            {"type":"LoraLoader","widgets_values":["style\\LORA.safetensors",1]},
            {"type":"H3C_Timeline","widgets_values":[{"files":["new.gguf","new.gguf"]}]},
            {"type":"H3C_SaveSegment","widgets_values":[]},
            {"type":"NewCoreNode","properties":{"cnr_id":"comfy-core"}}
        ]});
        let models = vec![
            drive("loras/lora.safetensors", ModelCategory::Loras),
            drive("models/loras/style/lora.safetensors", ModelCategory::Loras),
        ];
        let result = scan_workflow_models("h3", &graph.to_string(), &models).unwrap();
        assert_eq!(
            (result.nodeCount, result.presentCount, result.missingCount),
            (4, 1, 1)
        );
        assert_eq!(
            result.requiredCustomNodes,
            ["H3C_SaveSegment", "H3C_Timeline"]
        );
        assert_eq!(
            result.models[0].matchedDrivePath.as_deref(),
            Some("models/loras/style/lora.safetensors")
        );
        assert!(result.models[0].suggestedUrl.is_none());
        assert_eq!(result.models[1].category, ModelCategory::DiffusionModels);
    }

    #[test]
    fn api_and_wrapped_api_scan_all_extensions_and_escape_queries() {
        let graph = json!({"12":{"class_type":"VHS_VideoCombine","inputs":{"settings":["ignore.png","one.BIN","two.onnx",{"name":"space & 漫画.sft"}]}},
            "2":{"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name":"nested/BIG.ckpt"}},
            "3":{"class_type":"DualCLIPLoader","inputs":{"a":"enc.pt","b":"enc.pth"}}});
        for document in [graph.clone(), json!({"prompt":graph,"client_id":"a"})] {
            let result = scan_workflow_models(
                "test",
                &document.to_string(),
                &[drive(
                    "models/checkpoints/big.CKPT",
                    ModelCategory::Checkpoints,
                )],
            )
            .unwrap();
            assert_eq!(
                (result.nodeCount, result.presentCount, result.missingCount),
                (3, 1, 5)
            );
            assert_eq!(result.requiredCustomNodes, ["VHS_VideoCombine"]);
            let found = result
                .models
                .iter()
                .find(|m| m.filename.contains('&'))
                .unwrap();
            let url = Url::parse(found.suggestedUrl.as_ref().unwrap()).unwrap();
            assert_eq!(url.query_pairs().next().unwrap().1, "space & 漫画.sft");
        }
        for invalid in ["{", "null", "[]", "{\"x\":42}", "{\"nodes\":{}}"] {
            assert!(scan_workflow_models("", invalid, &[]).is_err());
        }
    }

    #[test]
    fn model_category_follows_specific_loader_before_architecture() {
        use ModelCategory::*;
        for (name, expected) in [
            ("LoraLoaderModelOnly", Loras),
            ("H3VideoVAELoader", Vae),
            ("QwenLoader", TextEncoders),
            ("DualCLIPLoader", TextEncoders),
            ("TextEncode", TextEncoders),
            ("T5Loader", TextEncoders),
            ("CLIPVisionLoader", ClipVision),
            ("UNETLoader", DiffusionModels),
            ("H3C_Timeline", DiffusionModels),
            ("DiffusionLoader", DiffusionModels),
            ("ControlNetLoader", Controlnet),
            ("UpscaleModelLoader", UpscaleModels),
            ("MiniMaxH3SemanticBridgeConfigT8", SemanticBridge),
            ("CheckpointLoaderSimple", Checkpoints),
        ] {
            assert_eq!(category(name), expected, "{name}");
        }
    }

    #[test]
    fn core_registry_and_declared_settings_contract() {
        assert!(builtin("Epsilon Scaling"));
        assert!(builtin("Video Slice"));
        assert!(!builtin("VHS_VideoCombine"));
        let spec = json!({"paths":{"/api/settings/colab":{"patch":{"requestBody":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/ColabConfig"}}}}}}},
            "components":{"schemas":{"ColabConfig":{"properties":{"endpoint_url":{"type":"string"},"applyToGlobal":{"type":"boolean"}}}}}});
        let (method, props) = update_contract(&spec).unwrap();
        assert_eq!(method, reqwest::Method::PATCH);
        assert_eq!(saved_endpoint(&props).unwrap().0, "endpoint_url");
        assert_eq!(
            saved_endpoint(&json!({"settings":{"baseUrl":"https://example.com"}}))
                .unwrap()
                .1,
            "https://example.com"
        );
    }

    #[test]
    fn gguf_complete_shards_projectors_and_precision_priority() {
        let mib = 1 << 20;
        let files = [
            ("M-UD-Q4_K_XL.gguf", 5000),
            ("M-Q6_K-00002-of-00002.gguf", 3500),
            ("M-Q6_K-00001-of-00002.gguf", 3500),
            ("M-Q8_0-00001-of-00002.gguf", 8000),
            ("M-IQ4_XS.gguf", 4999),
            ("mmproj-F16.gguf", 512),
            ("M-BF16.gguf", 15000),
        ];
        let metadata = json!({"siblings":files.iter().map(|(f,n)|json!({"rfilename":f,"size":*n as u64 * mib})).collect::<Vec<_>>()});
        let result = inspect_metadata("o/r", 16_000, &metadata).unwrap();
        assert_eq!(result.autoPickedQuant.as_deref(), Some("Q6_K"));
        assert_eq!(result.mmprojFiles, ["mmproj-F16.gguf"]);
        assert!(result.variants.iter().all(|v| v.quant != "Q8_0"));
        let best = result.variants.iter().find(|v| v.isAutoSelected).unwrap();
        assert_eq!(
            best.files,
            ["M-Q6_K-00001-of-00002.gguf", "M-Q6_K-00002-of-00002.gguf"]
        );
        assert_eq!(best.sizeBytes, 7000 * mib);
        assert_eq!(best.recommendedCtx, 16384);
        assert_eq!(best.estimatedVramMib, 7000 + 512 + 4096 + 768);
        assert!(result.variants.iter().any(|v| v.quant == "UD-Q4_K_XL"));
        let big = inspect_metadata("o/r", 97_000, &metadata).unwrap();
        assert_eq!(big.autoPickedQuant.as_deref(), Some("Q6_K"));
        assert_eq!(big.variants[0].recommendedCtx, 32768);
    }

    #[test]
    fn gguf_exact_memory_limit_full_precision_and_missing_file_sizes() {
        // 6684 MiB weights + 2048 MiB KV + 768 MiB overhead = 95% of 10000.
        let weights = 6684u64 << 20;
        let meta = json!({"siblings":[{"rfilename":"m-BF16.gguf","lfs":{"size":weights}},{"rfilename":"unknown-Q8_0.gguf"}]});
        let exact = inspect_metadata("o/r", 10000, &meta).unwrap();
        assert_eq!(exact.autoPickedQuant.as_deref(), Some("BF16"));
        assert_eq!(exact.variants[0].recommendedCtx, 8192);
        assert_eq!(exact.variants.len(), 1);
        assert!(inspect_metadata("o/r", 9999, &meta)
            .unwrap()
            .autoPickedQuant
            .is_none());
        assert_eq!(vram_mib(weights + 1, 8192), 9501);
        assert!(inspect_metadata("o/r", 0, &meta)
            .unwrap()
            .autoPickedQuant
            .is_none());
        let duplicate = json!({"siblings":[{"rfilename":"a-Q4_0-00001-of-00002.gguf","size":10},{"rfilename":"b-Q4_0-00002-of-00002.gguf","size":10}]});
        assert!(inspect_metadata("o/r", 10000, &duplicate)
            .unwrap()
            .variants
            .is_empty());
    }

    #[test]
    fn default_workflows_and_continuation_bind_real_graph_inputs() {
        let wf = default_saved_api_workflows();
        let mut first = apply_workflow_params(
            &wf[0],
            &HashMap::from([
                ("frames".into(), json!(243)),
                ("image".into(), json!("input.png")),
            ]),
        )
        .unwrap();
        configure_h3_sequence(&mut first, "test-shot", None).unwrap();
        let mut second = apply_workflow_params(
            &wf[0],
            &HashMap::from([
                ("frames".into(), json!(238)),
                ("prompt".into(), json!("next")),
                ("seed".into(), json!(7)),
            ]),
        )
        .unwrap();
        assert!(configure_h3_sequence(&mut second, "other", Some(&first)).is_err());
        configure_h3_sequence(&mut second, "test-shot", Some(&first)).unwrap();
        let timeline: Value =
            serde_json::from_str(second["100"]["inputs"]["timeline"].as_str().unwrap()).unwrap();
        assert_eq!(timeline["boxes"][0]["frames"], 243);
        assert_eq!(timeline["boxes"][1]["frames"], 238);
        assert_eq!(timeline["boxes"][1]["prompt"], "next");
        assert_eq!(timeline["boxes"][1]["seed"], 7);
        assert_eq!(second["6"]["inputs"]["image"], "input.png");
        let image =
            apply_workflow_params(&wf[1], &HashMap::from([("seed".into(), json!(12))])).unwrap();
        assert_eq!(image["3"]["inputs"]["seed"], 12);
        assert_eq!(
            scan_workflow_models("h3", &first.to_string(), &default_drive_models_seed())
                .unwrap()
                .presentCount,
            6
        );
        assert_eq!(default_workflow_profiles().len(), 4);
    }
}
