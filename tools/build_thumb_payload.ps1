param(
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,

    [switch]$ForceStereoDiagnostic,

    [switch]$RejectRecordingDiagnostic,

    [switch]$OccupancyProbeDiagnostic,

    [switch]$SkipContentIndexDiagnostic,

    [switch]$AcceptedIdleProbeDiagnostic,

    [switch]$OpenCloseProbeDiagnostic,

    [switch]$PumpCloseProbeDiagnostic,

    [switch]$HangAfterFirstCloseDiagnostic,

    [switch]$SyntheticCommitProbeDiagnostic,

    [switch]$DataCallbackCloseProbeDiagnostic,

    [switch]$AllDataCallbackCloseProbeDiagnostic,

    [switch]$DirectTerminalCompletionDiagnostic,

    [switch]$SinglePassLeftDiagnostic,

    [switch]$SinglePassRightDiagnostic,

    [switch]$KnownOutputToneDiagnostic,

    [switch]$FinalWriteToneDiagnostic,

    [switch]$AbortPathHangDiagnostic,

    [switch]$TerminalOddWordClampDiagnostic,

    [switch]$SourceReadRetryDiagnostic,

    [switch]$DataPathAbortPassthroughDiagnostic,

    [switch]$DataCallbackAbortPassthroughDiagnostic,

    [switch]$SafePairGuard,

    [switch]$SamplePairLedPoc,

    [switch]$SamplePairIdentityPoc,

    [switch]$SamplePairGesturePoc,

    [switch]$SamplePairWriterCouplingPoc,

    [switch]$SampleCacheRefreshPoc,

    [switch]$PairedPlaybackPoc,

    [switch]$PanParameterProbe,

    [switch]$AutoPanPoc,

    [switch]$StereoPositionPoc,
    [switch]$StereoStockSaveQueue,

    [switch]$MixerVolumeProbe,

    [switch]$WriterRequestProbeDiagnostic,

    [switch]$RollbackAfterRightSyncDiagnostic,

    [switch]$NativeDualCapturePoc,

    [switch]$NativeSlotMetadataProbeDiagnostic,

    [switch]$NativeCapturePreflightProbeDiagnostic,
    [switch]$VoiceChannelProbe,
    [switch]$MacroDelegateProbe,
    [switch]$CropExtentProbe,
    [switch]$CropIoTrace,
    [switch]$CropMirrorWindow,

    [switch]$NativeWriterReadinessProbeDiagnostic,

    [switch]$NativeWriterAcquisitionProbeDiagnostic,

    [switch]$NativeArmTokenProbeDiagnostic,

    [switch]$NativePackValueProbeDiagnostic,

    [switch]$NativePackCorrectionProbeDiagnostic,

    [switch]$NativeFinalizationProbeDiagnostic,

    [switch]$NativePublicationProbeDiagnostic,

    [switch]$NativeTerminalPublishProbeDiagnostic,

    [switch]$NativePublishBoundaryProbeDiagnostic,

    [switch]$NativePublishSlotProbeDiagnostic,

    [switch]$NativeDeferredRefreshProbeDiagnostic
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$rustRoot = Join-Path $root 'tools\vendor\rust'
$rustc = Join-Path $rustRoot 'cargo\bin\rustc.exe'
$linker = Get-ChildItem `
    (Join-Path $rustRoot 'rustup\toolchains') `
    -Recurse `
    -Filter 'rust-lld.exe' |
    Select-Object -First 1 -ExpandProperty FullName
$source = Join-Path $root 'patches\stereo-capture-arm-payload.rs'
$linkerScript = Join-Path $root 'patches\stereo-capture-arm-payload.ld'
$output = [System.IO.Path]::GetFullPath($OutputDirectory)

$env:RUSTUP_HOME = Join-Path $rustRoot 'rustup'
$env:CARGO_HOME = Join-Path $rustRoot 'cargo'

if (-not (Test-Path -LiteralPath $rustc)) {
    throw "rustc was not found at $rustc"
}
if (-not (Test-Path -LiteralPath $source)) {
    throw "Payload source was not found at $source"
}
if (-not $linker) {
    throw "rust-lld.exe was not found under $rustRoot"
}
if (-not (Test-Path -LiteralPath $linkerScript)) {
    throw "Linker script was not found at $linkerScript"
}

New-Item -ItemType Directory -Path $output -Force | Out-Null

$optimizationLevel = if ($NativeDualCapturePoc) { 'z' } else { 's' }
$rustcArguments = @(
    $source,
    '--target', 'thumbv7em-none-eabihf',
    '--crate-type', 'lib',
    '--emit', "asm=$output\stereo-capture-arm-payload.s,obj=$output\stereo-capture-arm-payload.o",
    '-C', "opt-level=$optimizationLevel",
    '-C', 'panic=abort',
    '-C', 'relocation-model=static'
)
if ($ForceStereoDiagnostic) {
    $rustcArguments += @('--cfg', 'force_stereo_diagnostic')
}
if ($RejectRecordingDiagnostic) {
    $rustcArguments += @('--cfg', 'reject_recording_diagnostic')
}
if ($OccupancyProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'occupancy_probe_diagnostic')
}
if ($SkipContentIndexDiagnostic) {
    $rustcArguments += @('--cfg', 'skip_content_index_diagnostic')
}
if ($AcceptedIdleProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'accepted_idle_probe_diagnostic')
}
if ($OpenCloseProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'open_close_probe_diagnostic')
}
if ($PumpCloseProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'pump_close_probe_diagnostic')
}
if ($HangAfterFirstCloseDiagnostic) {
    $rustcArguments += @('--cfg', 'hang_after_first_close_diagnostic')
}
if ($SyntheticCommitProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'synthetic_commit_probe_diagnostic')
}
if ($DataCallbackCloseProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'data_callback_close_probe_diagnostic')
}
if ($AllDataCallbackCloseProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'all_data_callback_close_probe_diagnostic')
}
if ($DirectTerminalCompletionDiagnostic) {
    $rustcArguments += @('--cfg', 'direct_terminal_completion_diagnostic')
}
if ($SinglePassLeftDiagnostic) {
    $rustcArguments += @('--cfg', 'single_pass_left_diagnostic')
}
if ($SinglePassRightDiagnostic) {
    $rustcArguments += @('--cfg', 'single_pass_right_diagnostic')
}
if ($KnownOutputToneDiagnostic) {
    $rustcArguments += @('--cfg', 'known_output_tone_diagnostic')
}
if ($FinalWriteToneDiagnostic) {
    $rustcArguments += @('--cfg', 'final_write_tone_diagnostic')
}
if ($AbortPathHangDiagnostic) {
    $rustcArguments += @('--cfg', 'abort_path_hang_diagnostic')
}
if ($TerminalOddWordClampDiagnostic) {
    $rustcArguments += @('--cfg', 'terminal_odd_word_clamp_diagnostic')
}
if ($SourceReadRetryDiagnostic) {
    $rustcArguments += @('--cfg', 'source_read_retry_diagnostic')
}
if ($DataPathAbortPassthroughDiagnostic) {
    $rustcArguments += @('--cfg', 'data_path_abort_passthrough_diagnostic')
}
if ($DataCallbackAbortPassthroughDiagnostic) {
    $rustcArguments += @('--cfg', 'data_callback_abort_passthrough_diagnostic')
}
if ($SafePairGuard) {
    $rustcArguments += @('--cfg', 'safe_pair_guard')
}
if ($SamplePairLedPoc) {
    $rustcArguments += @('--cfg', 'sample_pair_led_poc')
}
if ($SamplePairIdentityPoc) {
    $rustcArguments += @('--cfg', 'sample_pair_identity_poc')
}
if ($SamplePairGesturePoc) {
    $rustcArguments += @('--cfg', 'sample_pair_gesture_poc')
}
if ($SamplePairWriterCouplingPoc) {
    $rustcArguments += @('--cfg', 'sample_pair_writer_coupling_poc')
}
if ($SampleCacheRefreshPoc) {
    $rustcArguments += @('--cfg', 'sample_cache_refresh_poc')
}
if ($PairedPlaybackPoc) {
    $rustcArguments += @('--cfg', 'paired_playback_poc')
}
if ($PanParameterProbe) {
    $rustcArguments += @('--cfg', 'pan_parameter_probe')
}
if ($AutoPanPoc) {
    $rustcArguments += @('--cfg', 'auto_pan_poc')
}
if ($StereoStockSaveQueue) {
    $rustcArguments += @('--cfg', 'stereo_stock_save_queue')
}
if ($StereoPositionPoc) {
    $rustcArguments += @('--cfg', 'stereo_position_poc')
}
if ($MixerVolumeProbe) {
    $rustcArguments += @('--cfg', 'mixer_volume_probe')
}
if ($WriterRequestProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'writer_request_probe_diagnostic')
}
if ($RollbackAfterRightSyncDiagnostic) {
    $rustcArguments += @('--cfg', 'rollback_after_right_sync_diagnostic')
}
if ($NativeDualCapturePoc) {
    $rustcArguments += @('--cfg', 'native_dual_capture_poc')
}
if ($NativeSlotMetadataProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_slot_metadata_probe_diagnostic')
}
if ($NativeCapturePreflightProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_capture_preflight_probe_diagnostic')
}
if ($VoiceChannelProbe) {
    $rustcArguments += @('--cfg', 'voice_channel_probe')
}
if ($MacroDelegateProbe) {
    $rustcArguments += @('--cfg', 'macro_delegate_probe')
}
if ($CropExtentProbe) {
    $rustcArguments += @('--cfg', 'crop_extent_probe')
}
if ($CropIoTrace) {
    $rustcArguments += @('--cfg', 'crop_io_trace')
}
if ($CropMirrorWindow) {
    $rustcArguments += @('--cfg', 'crop_mirror_window')
}
if ($NativeWriterReadinessProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_writer_readiness_probe_diagnostic')
}
if ($NativeWriterAcquisitionProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_writer_acquisition_probe_diagnostic')
}
if ($NativeArmTokenProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_arm_token_probe_diagnostic')
}
if ($NativePackValueProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_pack_value_probe_diagnostic')
}
if ($NativePackCorrectionProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_pack_correction_probe_diagnostic')
}
if ($NativeFinalizationProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_finalization_probe_diagnostic')
}
if ($NativePublicationProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_publication_probe_diagnostic')
}
if ($NativeTerminalPublishProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_terminal_publish_probe_diagnostic')
}
if ($NativePublishBoundaryProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_publish_boundary_probe_diagnostic')
}
if ($NativePublishSlotProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_publish_slot_probe_diagnostic')
}
if ($NativeDeferredRefreshProbeDiagnostic) {
    $rustcArguments += @('--cfg', 'native_deferred_refresh_probe_diagnostic')
}

& $rustc @rustcArguments

if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

& $linker `
    -flavor gnu `
    --gc-sections `
    -T $linkerScript `
    "-Map=$output\stereo-capture-arm-payload.map" `
    -o "$output\stereo-capture-arm-payload.elf" `
    "$output\stereo-capture-arm-payload.o"

if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

& $linker `
    -flavor gnu `
    --gc-sections `
    --oformat=binary `
    -T $linkerScript `
    -o "$output\stereo-capture-arm-payload.bin" `
    "$output\stereo-capture-arm-payload.o"

if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Get-Item `
    "$output\stereo-capture-arm-payload.s", `
    "$output\stereo-capture-arm-payload.o", `
    "$output\stereo-capture-arm-payload.elf", `
    "$output\stereo-capture-arm-payload.bin", `
    "$output\stereo-capture-arm-payload.map" |
    Select-Object FullName, Length
