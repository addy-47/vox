fn calculate_audio_token_len(n_frames: usize) -> usize {
    let input_lengths = n_frames;
    let input_lengths_leave = input_lengths % 100;
    let feat_lengths = if input_lengths_leave > 0 {
        (input_lengths_leave - 1) / 2 + 1
    } else {
        0
    };
    
    let mut output_lengths = if feat_lengths > 0 {
        ((feat_lengths - 1) / 2 + 1 - 1) / 2 + 1
    } else {
        0
    };
    
    output_lengths += (input_lengths / 100) * 13;
    output_lengths
}

fn main() {
    let test_cases = vec![
        (150, 20),
        (100, 13),
        (200, 26),
        (50, 7),
        (3000, 390),
    ];
    
    for (frames, expected) in test_cases {
        let actual = calculate_audio_token_len(frames);
        println!("Frames: {}, Expected: {}, Actual: {}", frames, expected, actual);
        assert_eq!(actual, expected);
    }
    println!("All math test cases passed!");
}
