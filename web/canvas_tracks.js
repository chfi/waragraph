


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
