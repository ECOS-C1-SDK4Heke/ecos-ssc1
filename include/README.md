# TODO-List

当前主要是bindgen是依赖include/generated/autoconf.h做的

所以支持其他板子就是条件编译/包含其他板子的autoconf，或者对代码没用直接移除了wrapper.h的包含部分

毕竟，真正的编译链接是导入此项目的用户的程序的build.rs做的，且那个时候C程序的generated/autoconf.h依赖都是用户程序目录下make menoconfig生成的include下的文件，这里仅仅是bindgen生成对应的变量，比如：

`#define CONFIG_UART_BAUD_RATE 115200` -> `static CONFIG_UART_BAUD_RATE: u64 = 115200;`

之类的，确认后续代码没用实际上也就可以把这个依赖删掉了...，就是直接将wrapper.h里面的`@include ...`给删掉即可

> 改名：因为本来只想支持c1，其他板子又没有，所以就叫ecos ssc1了，也或许，当一个开端，名称就当历史遗留了...

而且，要想支持其他板子，记得将全局堆分配器里写死的RAM结束地址变成可变的配置项之类的...
