# Generates reproducible 16 kHz mono speech fixtures with the local Windows
# voice. These are latency fixtures, not a WER corpus: use real recordings for
# accuracy. They are gitignored.

param([string]$OutDir = (Join-Path $PSScriptRoot "fixtures"))

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Add-Type -AssemblyName System.Speech
$format = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(
    16000,
    [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,
    [System.Speech.AudioFormat.AudioChannel]::Mono
)

$short = "Draft turns speech into text at the cursor."
$medium = "Hold the shortcut, speak naturally, and release. The final transcript appears almost instantly, with no cloud round trip."
$long = @"
This is a longer dictation test. It includes names like Tirup Mehta, a date on October twentieth, and a short list of three items: speed, accuracy, and privacy. The sentence ends here. A second sentence checks punctuation, commas, and a question mark? Finally, this paragraph gives the engine enough audio to behave like a real note.
"@

$clips = [ordered]@{
    "tts-3s.wav"  = $short
    "tts-5s.wav"  = $medium
    "tts-15s.wav" = $long
    "tts-30s.wav" = ($long + " " + $long)
    "tts-60s.wav" = ($long + " " + $long + " " + $long + " " + $long)
}

foreach ($entry in $clips.GetEnumerator()) {
    $path = Join-Path $OutDir $entry.Key
    $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
    try {
        $synth.Rate = 0
        $synth.SetOutputToWaveFile($path, $format)
        $synth.Speak($entry.Value)
    } finally {
        $synth.Dispose()
    }
    $info = Get-Item $path
    Write-Host ("{0}: {1:N1} KB" -f $entry.Key, ($info.Length / 1KB))
}

Write-Host "fixtures in $OutDir"
