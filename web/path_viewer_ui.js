

export function addPathViewerEventHandlers(worker, path_viewer, canvas) {
    console.log("adding path viewer event handlers & glue");

    const coord_sys = path_viewer.coord_sys;

    canvas.addEventListener("mouseover", (event) => {

        let mx = event.clientX;
        let canv_width = canvas.width;

        console.log("clientX: " + mx);

        path_viewer.view.then((view) => {
            let { left, right } = view;

            let bp_pos = left + (mx / canvas.width) * (right - left + 1);
            console.log("mouse at " + bp_pos + " bp");
        });

    });
}
