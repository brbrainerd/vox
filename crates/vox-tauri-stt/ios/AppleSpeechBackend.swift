import Foundation
import Speech
import AVFoundation

/// On-device Apple Speech (ported from mental-tracker Capacitor plugin; no Capacitor types).
class AppleSpeechBackend: NSObject {
    static let shared = AppleSpeechBackend()
    private let recognizer = SFSpeechRecognizer(locale: Locale.current)
    private let audioEngine = AVAudioEngine()
    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?

    func transcribe(completion: @escaping (Result<(String, Float), Error>) -> Void) {
        SFSpeechRecognizer.requestAuthorization { status in
            DispatchQueue.main.async {
                guard status == .authorized else {
                    completion(.failure(NSError(
                        domain: "speech",
                        code: 1,
                        userInfo: [NSLocalizedDescriptionKey: "Speech recognition not authorized"]
                    )))
                    return
                }
                self.start(completion: completion)
            }
        }
    }

    private func start(completion: @escaping (Result<(String, Float), Error>) -> Void) {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.record, mode: .measurement, options: .duckOthers)
            try session.setActive(true, options: .notifyOthersOnDeactivation)
        } catch {
            completion(.failure(error))
            return
        }

        recognitionRequest = SFSpeechAudioBufferRecognitionRequest()
        recognitionRequest!.shouldReportPartialResults = false
        recognitionRequest!.requiresOnDeviceRecognition = true

        let inputNode = audioEngine.inputNode
        let format = inputNode.outputFormat(forBus: 0)
        inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
            self?.recognitionRequest?.append(buffer)
        }

        audioEngine.prepare()
        do {
            try audioEngine.start()
        } catch {
            cleanup()
            completion(.failure(error))
            return
        }

        recognitionTask = recognizer?.recognitionTask(with: recognitionRequest!) { [weak self] result, error in
            guard let self = self else { return }
            if let result = result, result.isFinal {
                let text = result.bestTranscription.formattedString
                let confidence = result.bestTranscription.segments
                    .map { $0.confidence }
                    .reduce(0, +) / Float(max(result.bestTranscription.segments.count, 1))
                self.cleanup()
                DispatchQueue.main.async { completion(.success((text, confidence))) }
            } else if let error = error {
                self.cleanup()
                DispatchQueue.main.async { completion(.failure(error)) }
            }
        }
    }

    private func cleanup() {
        audioEngine.stop()
        audioEngine.inputNode.removeTap(onBus: 0)
        recognitionRequest?.endAudio()
        recognitionRequest = nil
        recognitionTask = nil
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
}
