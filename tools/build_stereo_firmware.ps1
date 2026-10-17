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

    [switch]$StockDspDiagnostic,

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

    [switch]$NativeDeferredRefreshProbeDiagnostic,

    [int]$ProtocolVersion = 1840,

    [int]$ApplicationBuild = 5705
)

$ErrorActionPreference = 'Stop'

# The window mirror used to ride the loop-window hooks the trace installed, so
# it had to force the trace on. The hooks are now installed for either flag, so
# the mirror can ship on its own without the probe LED pages replacing the
# sample pads.

$root = Split-Path -Parent $PSScriptRoot
$output = [System.IO.Path]::GetFullPath($OutputDirectory)
$thumbOutput = Join-Path $output 'thumb'
$assembler = Join-Path $root 'tools\vendor\rust\cargo\bin\dsp56300-asm.exe'
$python = Join-Path $root '.venv\Scripts\python.exe'
$dspSource = Join-Path $root 'patches\stereo-capture-dsp-poc.s'
$dspLod = Join-Path $output 'stereo-capture-dsp-poc.lod'
$dspListing = Join-Path $output 'stereo-capture-dsp-poc.lst'
$nativeDspOutput = Join-Path $output 'native-dsp'

foreach ($path in @(
    $assembler,
    $python,
    $dspSource,
    (Join-Path $root 'circuitrhythm-firmware-5706.bin'),
    (Join-Path $root 'circuitrhythm-firmware-5706.syx')
)) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Required input was not found at $path"
    }
}

New-Item -ItemType Directory -Path $output -Force | Out-Null

if ($NativeDualCapturePoc) {
    $extractedDsp = Join-Path $nativeDspOutput 'stock'
    & $python `
        (Join-Path $PSScriptRoot 'extract_dsp_blocks.py') `
        (Join-Path $root 'circuitrhythm-firmware-5706.bin') `
        $extractedDsp
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    & $python `
        (Join-Path $PSScriptRoot 'build_dual_native_mono_dsp.py') `
        --image (Join-Path $root 'circuitrhythm-firmware-5706.bin') `
        --primary-b (Join-Path $extractedDsp 'primary_b.dsp24') `
        --output-dir $nativeDspOutput
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} else {
    & $assembler `
        --format lod `
        --output $dspLod `
        --listing $dspListing `
        $dspSource
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

& (Join-Path $PSScriptRoot 'build_thumb_payload.ps1') `
    -OutputDirectory $thumbOutput `
    -ForceStereoDiagnostic:$ForceStereoDiagnostic `
    -RejectRecordingDiagnostic:$RejectRecordingDiagnostic `
    -OccupancyProbeDiagnostic:$OccupancyProbeDiagnostic `
    -SkipContentIndexDiagnostic:$SkipContentIndexDiagnostic `
    -AcceptedIdleProbeDiagnostic:$AcceptedIdleProbeDiagnostic `
    -OpenCloseProbeDiagnostic:$OpenCloseProbeDiagnostic `
    -PumpCloseProbeDiagnostic:$PumpCloseProbeDiagnostic `
    -HangAfterFirstCloseDiagnostic:$HangAfterFirstCloseDiagnostic `
    -SyntheticCommitProbeDiagnostic:$SyntheticCommitProbeDiagnostic `
    -DataCallbackCloseProbeDiagnostic:$DataCallbackCloseProbeDiagnostic `
    -AllDataCallbackCloseProbeDiagnostic:$AllDataCallbackCloseProbeDiagnostic `
    -DirectTerminalCompletionDiagnostic:$DirectTerminalCompletionDiagnostic `
    -SinglePassLeftDiagnostic:$SinglePassLeftDiagnostic `
    -SinglePassRightDiagnostic:$SinglePassRightDiagnostic `
    -KnownOutputToneDiagnostic:$KnownOutputToneDiagnostic `
    -FinalWriteToneDiagnostic:$FinalWriteToneDiagnostic `
    -AbortPathHangDiagnostic:$AbortPathHangDiagnostic `
    -TerminalOddWordClampDiagnostic:$TerminalOddWordClampDiagnostic `
    -SourceReadRetryDiagnostic:$SourceReadRetryDiagnostic `
    -DataPathAbortPassthroughDiagnostic:$DataPathAbortPassthroughDiagnostic `
    -DataCallbackAbortPassthroughDiagnostic:$DataCallbackAbortPassthroughDiagnostic `
    -SafePairGuard:$SafePairGuard `
    -SamplePairLedPoc:$SamplePairLedPoc `
    -SamplePairIdentityPoc:$SamplePairIdentityPoc `
    -SamplePairGesturePoc:$SamplePairGesturePoc `
    -SamplePairWriterCouplingPoc:$SamplePairWriterCouplingPoc `
    -SampleCacheRefreshPoc:$SampleCacheRefreshPoc `
    -PairedPlaybackPoc:$PairedPlaybackPoc `
    -PanParameterProbe:$PanParameterProbe `
    -AutoPanPoc:$AutoPanPoc `
    -StereoPositionPoc:$StereoPositionPoc `
    -StereoStockSaveQueue:$StereoStockSaveQueue `
    -MixerVolumeProbe:$MixerVolumeProbe `
    -WriterRequestProbeDiagnostic:$WriterRequestProbeDiagnostic `
    -RollbackAfterRightSyncDiagnostic:$RollbackAfterRightSyncDiagnostic `
    -NativeDualCapturePoc:$NativeDualCapturePoc `
    -NativeSlotMetadataProbeDiagnostic:$NativeSlotMetadataProbeDiagnostic `
    -NativeCapturePreflightProbeDiagnostic:$NativeCapturePreflightProbeDiagnostic `
    -VoiceChannelProbe:$VoiceChannelProbe `
    -MacroDelegateProbe:$MacroDelegateProbe `
    -CropExtentProbe:$CropExtentProbe `
    -CropIoTrace:$CropIoTrace `
    -CropMirrorWindow:$CropMirrorWindow `
    -NativeWriterReadinessProbeDiagnostic:$NativeWriterReadinessProbeDiagnostic `
    -NativeWriterAcquisitionProbeDiagnostic:$NativeWriterAcquisitionProbeDiagnostic `
    -NativeArmTokenProbeDiagnostic:$NativeArmTokenProbeDiagnostic `
    -NativePackValueProbeDiagnostic:$NativePackValueProbeDiagnostic `
    -NativePackCorrectionProbeDiagnostic:$NativePackCorrectionProbeDiagnostic `
    -NativeFinalizationProbeDiagnostic:$NativeFinalizationProbeDiagnostic `
    -NativePublicationProbeDiagnostic:$NativePublicationProbeDiagnostic `
    -NativeTerminalPublishProbeDiagnostic:$NativeTerminalPublishProbeDiagnostic `
    -NativePublishBoundaryProbeDiagnostic:$NativePublishBoundaryProbeDiagnostic `
    -NativePublishSlotProbeDiagnostic:$NativePublishSlotProbeDiagnostic `
    -NativeDeferredRefreshProbeDiagnostic:$NativeDeferredRefreshProbeDiagnostic
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

& $python `
    (Join-Path $PSScriptRoot 'build_stereo_firmware.py') `
    --image (Join-Path $root 'circuitrhythm-firmware-5706.bin') `
    --template (Join-Path $root 'circuitrhythm-firmware-5706.syx') `
    --dsp-lod $dspLod `
    --payload (Join-Path $thumbOutput 'stereo-capture-arm-payload.bin') `
    --payload-map (Join-Path $thumbOutput 'stereo-capture-arm-payload.map') `
    --output-image (Join-Path $output 'circuitrhythm-firmware-5706-stereo-poc.bin') `
    --output-sysex (Join-Path $output 'circuitrhythm-firmware-5706-stereo-poc.syx') `
    --manifest (Join-Path $output 'circuitrhythm-firmware-5706-stereo-poc.json') `
    --protocol-version $ProtocolVersion `
    --application-build $ApplicationBuild `
    $(if ($FinalWriteToneDiagnostic) { '--final-write-tone' }) `
    $(if ($SafePairGuard) { '--safe-pair-guard' }) `
    $(if ($SamplePairLedPoc) { '--sample-pair-led-poc' }) `
    $(if ($SamplePairIdentityPoc) { '--sample-pair-identity-poc' }) `
    $(if ($SamplePairGesturePoc) { '--sample-pair-gesture-poc' }) `
    $(if ($SamplePairWriterCouplingPoc) { '--sample-pair-writer-coupling-poc' }) `
    $(if ($SampleCacheRefreshPoc) { '--sample-cache-refresh-poc' }) `
    $(if ($PairedPlaybackPoc) { '--paired-playback-poc' }) `
    $(if ($StereoPositionPoc) { '--stereo-position-poc' }) `
    $(if ($StereoStockSaveQueue) { '--stereo-stock-save-queue' }) `
    $(if ($CropIoTrace) { '--crop-io-trace' }) `
    $(if ($CropMirrorWindow) { '--crop-mirror-window' }) `
    $(if ($MixerVolumeProbe) { '--mixer-volume-probe' }) `
    $(if ($WriterRequestProbeDiagnostic) { '--writer-request-probe-diagnostic' }) `
    $(if ($NativeDualCapturePoc) { '--native-dual-capture-poc' }) `
    $(if ($NativeCapturePreflightProbeDiagnostic) { '--native-capture-preflight-probe-diagnostic' }) `
    $(if ($NativeWriterReadinessProbeDiagnostic) { '--native-writer-readiness-probe-diagnostic' }) `
    $(if ($NativeWriterAcquisitionProbeDiagnostic) { '--native-writer-acquisition-probe-diagnostic' }) `
    $(if ($NativeArmTokenProbeDiagnostic) { '--native-arm-token-probe-diagnostic' }) `
    $(if ($NativePackValueProbeDiagnostic) { '--native-pack-value-probe-diagnostic' }) `
    $(if ($NativePackCorrectionProbeDiagnostic) { '--native-pack-correction-probe-diagnostic' }) `
    $(if ($NativeFinalizationProbeDiagnostic) { '--native-finalization-probe-diagnostic' }) `
    $(if ($NativePublicationProbeDiagnostic) { '--native-publication-probe-diagnostic' }) `
    $(if ($NativeTerminalPublishProbeDiagnostic) { '--native-terminal-publish-probe-diagnostic' }) `
    $(if ($NativePublishBoundaryProbeDiagnostic) { '--native-publish-boundary-probe-diagnostic' }) `
    $(if ($NativePublishSlotProbeDiagnostic) { '--native-publish-slot-probe-diagnostic' }) `
    $(if ($NativeDeferredRefreshProbeDiagnostic) { '--native-deferred-refresh-probe-diagnostic' }) `
    $(if ($NativeDualCapturePoc) { '--native-dsp-package'; (Join-Path $nativeDspOutput 'dual-native-mono-dsp-extension.dsp24') }) `
    $(if ($NativeDualCapturePoc) { '--native-dsp-metadata'; (Join-Path $nativeDspOutput 'dual-native-mono-dsp-patch.json') }) `
    $(if ($StockDspDiagnostic) { '--stock-dsp' })
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
