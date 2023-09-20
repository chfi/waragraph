

class OverviewMap {
    constructor(coord_sys, canvas) {
        this.coord_sys = coord_sys;
        this.canvas = canvas;
        this.latest_view = null;
    }

    draw(view) {
        const ctx = this.canvas.getContext('2d');

        let c_width = this.canvas.width;
        let max = view.max;

        let x0 = (view.start / max) * c_width;
        let x1 = (view.end / max) * c_width;

        ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);

        ctx.strokeStyle = "red";
        ctx.lineWidth = 2.0;

        ctx.rect(x0, -5.0, x1 - x0, this.canvas.height + 10.0);

        this.latest_view = view;
    }
}
