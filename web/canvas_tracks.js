


// function createHorizontalArrowPattern(head_height, head_length, separation) {
function createHorizontalArrowPattern() {
    const canvas = new OffscreenCanvas(32, 32);
    const ctx = canvas.getContext('2d');

    ctx.strokeStyle = 'black';

    ctx.moveTo(0, 16);
    ctx.lineTo(32, 16);

    ctx.moveTo(8, 16);
    ctx.lineTo(5, 8);
    ctx.moveTo(8, 16);
    ctx.lineTo(5, 24);

    // ctx.moveTo(16, 16);
    // ctx.lineTo(13, 8);
    // ctx.moveTo(16, 16);
    // ctx.lineTo(13, 24);

    ctx.moveTo(24, 16);
    ctx.lineTo(21, 8);
    ctx.moveTo(24, 16);
    ctx.lineTo(21, 24);

    ctx.stroke();

    return ctx.createPattern(canvas, "repeat-x");
}


const ARROW_PATTERN = createHorizontalArrowPattern();

export function drawVariableThicknessTrack(canvas, x_ranges) {
    const ctx = canvas.getContext('2d');
    ctx.save();

    const y_mid = canvas.height / 2;

    for (const entry of x_ranges) {
        const { start, end, thickness } = entry;
        ctx.fillRect(start, y_mid - thickness * 0.5, end - start, thickness);
    }

    ctx.restore();
}

export function drawBinaryArrowTrack(canvas, x_ranges) {
    const ctx = canvas.getContext('2d');
    ctx.save();

    console.log(ARROW_PATTERN);

    const y_mid = canvas.height / 2;

    for (const entry of x_ranges) {
        const { start, end, thick } = entry;

        if (thick) {
            ctx.fillStyle = 'black';
        } else {
            ctx.fillStyle = ARROW_PATTERN;
        }

        ctx.fillRect(start, y_mid - 16, end - start, 24);
    }

    ctx.restore();
}



export function drawSequence(canvas, sequence, subpixel_offset) {
    const ctx = canvas.getContext('2d');
    ctx.save();

    let view_len = sequence.length;
    let width = canvas.width;

    let bp_width = width / view_len;

    let y = canvas.height / 2;

    ctx.font = "12px monospace";
    ctx.textAlign = "center";

    let base_i = 0;

    for (const base of sequence) {
        let txt = typeof base === 'string' ? base : String.fromCharCode(base);

        let x = base_i * bp_width + 0.5 * bp_width;
        ctx.fillText(txt, x, y);

        base_i += 1;
    }

    ctx.restore();
}


/*
  entries is an array of visualization space ranges and color
*/
export function createHighlightCallback(entries) {
    console.log(">>>>>>> createHighlightCallback");
    console.log(entries);

    return (canvas, view) => {
        const ctx = canvas.getContext('2d');

        const view_len = view.end - view.start;

        ctx.save();
        console.log(entries.length);
        for (const { start, end, color } of entries) {
            let x_start = ((start - view.start) / view_len) * canvas.width;
            let x_end = ((end - view.start) / view_len) * canvas.width;
            let len = x_end - x_start;

            if (len > 1.0) {
                ctx.globalAlpha = 1.0;

                // let c = 'black'
                ctx.fillStyle = color;
                ctx.strokeStyle = color;

                let h = canvas.height;

                ctx.beginPath();
                // ctx.fillRect(x_start, 0.1 * h, x_end - x_start, 0.8 * h);
                ctx.clearRect(x_start, 0.1 * h, x_end - x_start, 0.8 * h);
                ctx.rect(x_start, 0.1 * h, x_end - x_start, 0.8 * h);
                ctx.fill();
                // ctx.closePath();
                // ctx.strokeRect(x_start, 0.1 * h, x_end - x_start, 0.8 * h);
            }
        }
        ctx.restore();

    };
}
