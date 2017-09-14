.PHONY: compile clean

%.vert.spv: %.glsl
	glslangValidator -S vert -V -o $@ $<

%.frag.spv: %.glsl
	glslangValidator -S frag -V -o $@ $<

compile: vertex.vert.spv fragment.frag.spv

clean:
	-rm vertex.vert.spv
	-rm fragment.frag.spv
