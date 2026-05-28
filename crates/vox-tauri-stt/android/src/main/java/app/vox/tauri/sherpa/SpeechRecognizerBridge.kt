package app.vox.tauri.sherpa

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import org.json.JSONObject

/**
 * On-device transcription via [SpeechRecognizer] (offline when OEM packs allow).
 * Ported from the legacy Capacitor plugin — no Capacitor symbols.
 * Host apps call this from Rust JNI or a thin Tauri Android plugin wrapper.
 */
object SpeechRecognizerBridge {

    fun transcribe(
        context: Context,
        onResult: (JSONObject) -> Unit,
        onError: (String) -> Unit,
    ) {
        if (!SpeechRecognizer.isRecognitionAvailable(context)) {
            onError("Speech recognition is not available on this device")
            return
        }
        val speechRecognizer = SpeechRecognizer.createSpeechRecognizer(context)
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(
                RecognizerIntent.EXTRA_LANGUAGE_MODEL,
                RecognizerIntent.LANGUAGE_MODEL_FREE_FORM,
            )
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, false)
            putExtra(RecognizerIntent.EXTRA_PREFER_OFFLINE, true)
        }

        speechRecognizer.setRecognitionListener(object : RecognitionListener {
            override fun onReadyForSpeech(params: Bundle?) {}
            override fun onBeginningOfSpeech() {}
            override fun onRmsChanged(rmsdB: Float) {}
            override fun onBufferReceived(buffer: ByteArray?) {}
            override fun onEndOfSpeech() {}

            override fun onError(error: Int) {
                speechRecognizer.destroy()
                onError("SpeechRecognizer error code: $error")
            }

            override fun onResults(results: Bundle?) {
                val matches = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                val confidences = results?.getFloatArray(SpeechRecognizer.CONFIDENCE_SCORES)
                val text = matches?.firstOrNull().orEmpty()
                val out = JSONObject()
                out.put("text", text)
                if (confidences != null && confidences.isNotEmpty()) {
                    out.put("confidence", confidences[0].toDouble())
                }
                speechRecognizer.destroy()
                onResult(out)
            }

            override fun onPartialResults(partialResults: Bundle?) {}
            override fun onEvent(eventType: Int, params: Bundle?) {}
        })

        speechRecognizer.startListening(intent)
    }
}
